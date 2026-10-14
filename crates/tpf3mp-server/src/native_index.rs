//! The native mods this server offers its players (proposed D29 amendment,
//! docs/NATIVE_MODS.md, "A server's own index"): an index made from a
//! registry, a git repository listing the mods' own repositories, read
//! again every so often, and the mods its open pull requests propose. For
//! mods on GitHub, the index also says how often each was downloaded and
//! when it last changed, as GitHub tells it.
//!
//! The operator names the registry (`--native-mods-registry`); without one
//! the server offers none (`Response::NativeIndex(None)`). The index is
//! built off the async runtime and swapped in whole, so a client reading
//! its pages reads one index, and one whose registry cannot be read keeps
//! the last that could.

use std::{
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tpf3mp_nativemods::{fetch, registry};
use tpf3mp_proto::NativeIndexPage;
use tracing::{info, warn};

/// Where the server's native mods come from.
#[derive(Debug, Clone)]
pub struct NativeRegistry {
    /// The registry's repository (`https://github.com/<owner>/<name>`), or
    /// the address of its list file.
    pub url: String,
    /// How often it is read again.
    pub every: Duration,
    /// A GitHub token for reading its open pull requests and its mods'
    /// releases, past GitHub's limit for unnamed callers. Optional.
    pub github_token: Option<String>,
}

/// The pages of one index.
type Pages = Arc<Vec<NativeIndexPage>>;

/// What the server serves now.
#[derive(Debug, Default)]
pub struct NativeIndex {
    accepted: RwLock<Option<Pages>>,
    proposed: RwLock<Option<Pages>>,
}

impl NativeIndex {
    /// Page `page` of the index, or with `proposed` of the proposals; none
    /// while the server has none.
    pub fn page(&self, proposed: bool, page: u16) -> Option<NativeIndexPage> {
        let held = if proposed {
            &self.proposed
        } else {
            &self.accepted
        };
        let pages = held
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()?;
        pages.get(usize::from(page)).cloned().or_else(|| {
            // Past the end: the first page says how many there are.
            pages.first().cloned()
        })
    }

    fn set(&self, proposed: bool, pages: Option<Pages>) {
        let held = if proposed {
            &self.proposed
        } else {
            &self.accepted
        };
        *held
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = pages;
    }

    /// Reads `registry` now and every so often after, for as long as the
    /// runtime runs.
    pub fn spawn(self: &Arc<Self>, registry: NativeRegistry) {
        let served = Arc::clone(self);
        tokio::spawn(async move {
            let mut serial = 0;
            loop {
                let reading = registry.clone();
                let built = tokio::task::spawn_blocking(move || read(&reading, serial)).await;
                match built {
                    Ok(Some(read)) => {
                        serial = read.serial;
                        served.set(false, Some(Arc::new(read.accepted)));
                        served.set(true, Some(Arc::new(read.proposed)));
                    }
                    Ok(None) => {}
                    Err(error) => warn!(%error, "reading the native mods' registry failed"),
                }
                tokio::time::sleep(registry.every).await;
            }
        });
    }
}

/// An index and its proposals, read and paged.
struct Read {
    serial: u64,
    accepted: Vec<NativeIndexPage>,
    proposed: Vec<NativeIndexPage>,
}

const USER_AGENT: &str = concat!("TPF3-MP-server/", env!("CARGO_PKG_VERSION"));

/// Largest answer read from GitHub's API.
const MAX_API_ANSWER: u64 = 1 << 20;

/// Reads the registry: its list, every mod's file, what GitHub says of the
/// mods, its open pull requests. `None` when its list cannot be read.
fn read(registry: &NativeRegistry, previous: u64) -> Option<Read> {
    let agent = fetch::agent(true, Duration::from_secs(60));
    let get = |url: &str, limit: u64| {
        fetch::get(&agent, url, limit, USER_AGENT).map_err(|error| error.to_string())
    };
    // GitHub's API: its own media type, and the operator's token if any.
    let api = |url: &str| {
        let mut request = agent
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = &registry.github_token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        request
            .call()
            .map_err(|e| e.to_string())
            .and_then(|mut response| {
                response
                    .body_mut()
                    .with_config()
                    .limit(MAX_API_ANSWER)
                    .read_to_vec()
                    .map_err(|e| e.to_string())
            })
    };
    let list_url = registry::raw_url(&registry.url, registry::LIST_FILE)?;
    let list = match get(&list_url, registry::MAX_LIST)
        .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
    {
        Ok(list) => list,
        Err(error) => {
            warn!(%error, registry = %registry.url, "cannot read the native mods' registry; the last index stays");
            return None;
        }
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let serial = registry::next_serial(previous, now);
    let mut built = registry::build(&list, serial, get);
    for problem in &built.problems {
        warn!(%problem, "a native mod is left out of the index");
    }
    let (listings, problems) = registry::listings(&built.sources, api);
    for problem in &problems {
        warn!(%problem, "cannot read a native mod's downloads and date");
    }
    built.add_listings(listings);
    let pulls = registry::pulls_url(&registry.url)
        .map(|url| {
            api(&url)
                .and_then(|json| registry::parse_pulls(&json))
                .unwrap_or_else(|error| {
                    warn!(%error, "cannot read the registry's open pull requests");
                    Vec::new()
                })
        })
        .unwrap_or_default();
    let (proposals, proposed_json, problems) =
        registry::propose(&list, &built.index, &pulls, serial, get);
    for problem in &problems {
        warn!(%problem, "a proposed native mod is left out");
    }
    info!(
        serial,
        mods = built.index.packages.len(),
        proposed = proposals.packages.len(),
        "read the native mods' registry"
    );
    Some(Read {
        serial,
        accepted: NativeIndexPage::split(serial, &built.json)?,
        proposed: NativeIndexPage::split(serial, &proposed_json)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_come_from_one_index_and_none_without_one() {
        let served = NativeIndex::default();
        assert_eq!(served.page(false, 0), None);
        let json = vec![b'x'; tpf3mp_proto::NATIVE_INDEX_PAGE + 10];
        served.set(
            false,
            Some(Arc::new(NativeIndexPage::split(9, &json).unwrap())),
        );
        let second = served.page(false, 1).unwrap();
        assert_eq!((second.serial, second.page, second.pages), (9, 1, 2));
        assert_eq!(second.bytes.len(), 10);
        assert_eq!(served.page(false, 7).unwrap().page, 0);
        assert_eq!(served.page(true, 0), None, "no proposals yet");
    }
}
