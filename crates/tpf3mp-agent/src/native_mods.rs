//! Native mods (D29, docs/NATIVE_MODS.md): what the launcher hands the game
//! it starts, what it declares to a room, and the player's panel of them.
//!
//! Before the game starts, the launcher writes the enabled packages for the
//! game's build (the executable's SHA-256) to `native-mods/enabled.json` in
//! its data folder and names that file in the game's environment
//! (`TPF3MP_NATIVE_MODS`); the hook enables nothing else. With nothing
//! enabled, or no native mod installed, the game gets no variable and the
//! executable is not even hashed.
//!
//! The simulation-changing ones among them are the game's native terms
//! ([`tpf3mp_nativemods::terms`]): the launcher declares them in the content
//! manifest, with their settings for a room's owner ([`NativeMod`]), and a
//! member who joins a room whose terms differ gets the room's versions from
//! the signed index and the room's settings ([`match_room`]), never from
//! another player. Mods from Mod Hub are not native: players get those
//! through their own game (D28).
//!
//! Downloads need the launcher's HTTPS source ([`NativeSetup`]); this crate
//! builds without one, and a launcher built without the native-mods key
//! offers no native mod at all.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::SystemTime,
};

use serde::Serialize;
use tpf3mp_nativemods::{
    FOLDER, enabled,
    enabled::Enabled,
    features::{self, BUILT_IN, Effect},
    fetch::sha256_of_file,
    index::{Index, Package, Setting},
    registry::{Proposal, Proposals},
    resolve,
    store::{Source, Store, StoreError},
    terms,
};
use tpf3mp_proto::{NativeMod, NativeSetting, NativeValue, Text};
use tracing::{info, warn};

/// Where the launcher gets native mods: the project's signed index, as the
/// launcher binary fetches it, and the keys it is signed with.
#[derive(Clone)]
pub struct NativeSetup {
    pub source: Arc<dyn Source + Send + Sync>,
    pub keys: Vec<Vec<u8>>,
}

impl std::fmt::Debug for NativeSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSetup")
            .field("keys", &self.keys.len())
            .finish_non_exhaustive()
    }
}

/// What the game the launcher starts gets of the native mods.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameNative {
    /// The variable naming the enabled list, if any is enabled.
    pub env: Option<(String, String)>,
    /// The simulation-changing ones, as the room compares them.
    pub terms: Vec<NativeMod>,
}

fn store(data_dir: &Path) -> Result<Store, String> {
    Store::open(data_dir.join(FOLDER)).map_err(|error| format!("native mods: {error}"))
}

/// The SHA-256 of the game's executable `exe`, computed once for each
/// version of the file.
pub fn build_of(exe: &Path) -> Result<String, String> {
    type Seen = Option<(PathBuf, u64, Option<SystemTime>, String)>;
    static SEEN: Mutex<Seen> = Mutex::new(None);
    let meta = std::fs::metadata(exe)
        .map_err(|error| format!("native mods: cannot read {}: {error}", exe.display()))?;
    let (len, modified) = (meta.len(), meta.modified().ok());
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((path, l, m, build)) = seen.as_ref()
        && path == exe
        && *l == len
        && *m == modified
    {
        return Ok(build.clone());
    }
    let build = sha256_of_file(exe)
        .map_err(|error| format!("native mods: cannot hash {}: {error}", exe.display()))?;
    *seen = Some((exe.to_owned(), len, modified, build.clone()));
    Ok(build)
}

/// The variable for the game's environment naming the native mods enabled
/// for `exe`, with the launcher's data folder `data_dir`, and their terms.
/// A registry that cannot be read stops the start: what it enables cannot
/// be told (fail closed).
pub fn game_env(data_dir: &Path, exe: &Path) -> Result<GameNative, String> {
    let store = store(data_dir)?;
    if !store.registry().packages.values().any(|i| i.enabled) {
        return Ok(GameNative::default());
    }
    let build = build_of(exe)?;
    let (path, left_out) = store
        .write_enabled(&build)
        .map_err(|error| format!("native mods: {error}"))?;
    for id in left_out {
        warn!(%id, "the native mod is enabled but not for this build of the game; left out");
    }
    info!(file = %path.display(), "the game gets the enabled native mods");
    let list = enabled::read(&path)?;
    Ok(GameNative {
        env: Some((enabled::ENV.to_owned(), path.to_string_lossy().into_owned())),
        terms: room_terms(&store, &list)?,
    })
}

/// The terms the next game on `exe` runs with, as [`game_env`] would hand
/// them to it, without writing anything.
pub fn next_terms(data_dir: &Path, exe: &Path) -> Result<Vec<NativeMod>, String> {
    let store = store(data_dir)?;
    if !store.registry().packages.values().any(|i| i.enabled) {
        return Ok(Vec::new());
    }
    let (list, _) = store
        .enabled_for(&build_of(exe)?)
        .map_err(|error| format!("native mods: {error}"))?;
    room_terms(&store, &list)
}

/// The simulation-changing packages of `list` as a room's native mods.
fn room_terms(store: &Store, list: &Enabled) -> Result<Vec<NativeMod>, String> {
    terms::terms(list, BUILT_IN)
        .into_iter()
        .map(|term| {
            let package = list
                .packages
                .iter()
                .find(|p| p.id == term.id)
                .ok_or_else(|| format!("native mods: {} is not enabled", term.id))?;
            let name = store
                .registry()
                .packages
                .get(&term.id)
                .and_then(|i| i.package())
                .map_or(term.id.as_str(), |p| p.name.as_str());
            let settings = package
                .settings
                .iter()
                .map(|(key, value)| {
                    Ok(NativeSetting {
                        key: Text::new(key)
                            .map_err(|_| format!("the setting {key} is too long"))?,
                        value: to_value(value)?,
                    })
                })
                .collect::<Result<_, String>>()?;
            Ok(NativeMod {
                id: Text::new(&term.id).map_err(|_| format!("the id {} is too long", term.id))?,
                version: Text::new(term.mod_version())
                    .map_err(|_| format!("the version of {} is too long", term.id))?,
                name: Text::lossy(name),
                settings,
            })
        })
        .collect()
}

/// A setting as a room tells it.
pub fn to_value(setting: &Setting) -> Result<NativeValue, String> {
    Ok(match setting {
        Setting::Bool(on) => NativeValue::Bool(*on),
        Setting::Int(n) => NativeValue::Int(*n),
        Setting::Text(text) => NativeValue::Text(
            Text::new(text).map_err(|_| "a native mod's text setting is too long".to_owned())?,
        ),
    })
}

/// A setting a room told, as the store keeps it.
pub fn from_value(value: &NativeValue) -> Setting {
    match value {
        NativeValue::Bool(on) => Setting::Bool(*on),
        NativeValue::Int(n) => Setting::Int(*n),
        NativeValue::Text(text) => Setting::Text(text.as_str().to_owned()),
    }
}

/// The native mods panel: every package of the server's index and of the
/// last signed index for this game's build, every one installed, and when
/// the player asks, the mods proposed for the server's registry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NativeView {
    /// Whether there is an index to offer mods from: the server played on
    /// offers one, or this launcher was built with the native-mods key.
    pub available: bool,
    /// The serial of the index shown, once one was accepted.
    pub serial: Option<u64>,
    /// The server whose index is shown, if any.
    pub server: Option<String>,
    /// Whether the mods proposed for the server's registry are shown.
    pub proposed_shown: bool,
    pub packages: Vec<NativeRow>,
    /// Why the panel could not be read, if it could not.
    pub problem: Option<String>,
}

/// One native mod in the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeRow {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The mod's own project page, to open in the browser.
    pub homepage: Option<String>,
    /// Whether every game of a room must run it alike.
    pub simulation: bool,
    /// The newest version the index has for this game's build.
    pub available: Option<String>,
    pub installed: Option<String>,
    /// The version a rollback goes back to.
    pub previous: Option<String>,
    pub enabled: bool,
    /// Whether the index still lists it.
    pub listed: bool,
    pub settings: Vec<NativeSettingRow>,
    /// For a mod only proposed for the server's registry: its pull request.
    /// Nobody has reviewed it.
    pub proposed: Option<Proposal>,
}

/// One setting of an installed native mod: the player's value, else the
/// default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeSettingRow {
    pub key: String,
    pub value: Setting,
    pub default: Setting,
}

/// Whether `package` changes the simulation: it says so, or a feature it
/// names does (an unknown one counts, fail closed).
fn changes_simulation(package: &Package) -> bool {
    package.simulation
        || package
            .features
            .iter()
            .any(|id| features::find(BUILT_IN, id).is_none_or(|f| f.effect == Effect::Simulation))
}

/// Where the launcher keeps a server's index: `native-mods/servers/<the
/// start of the SHA-256 of its address>/`.
fn server_dir(data_dir: &Path, server: &str) -> PathBuf {
    let key = tpf3mp_nativemods::signed::sha256_hex(server.trim().to_ascii_lowercase().as_bytes());
    data_dir.join(FOLDER).join("servers").join(&key[..16])
}

const SERVER_INDEX: &str = "index.json";
const SERVER_PROPOSED: &str = "proposed.json";

/// Takes in `json`, the index the server `server` serves with `serial` (or
/// with `proposed`, the mods proposed for it), and keeps it: the server
/// vouches for it as it vouches for its rooms (proposed D29 amendment). An
/// index whose serial is older than the one kept is refused, so an old one
/// cannot come back.
pub fn accept_server_index(
    data_dir: &Path,
    server: &str,
    serial: u64,
    json: &[u8],
    proposed: bool,
) -> Result<(), String> {
    let found = if proposed {
        Proposals::parse(json).map(|p| p.serial)
    } else {
        Index::parse(json)
            .map(|index| index.serial)
            .map_err(|error| error.to_string())
    }
    .map_err(|error| format!("{server}'s native mods: {error}"))?;
    if found != serial {
        return Err(format!("{server}'s native mods name another serial"));
    }
    let dir = server_dir(data_dir, server);
    let file = dir.join(if proposed {
        SERVER_PROPOSED
    } else {
        SERVER_INDEX
    });
    let kept = std::fs::read(&file).ok().and_then(|bytes| {
        if proposed {
            Proposals::parse(&bytes).ok().map(|p| p.serial)
        } else {
            Index::parse(&bytes).ok().map(|index| index.serial)
        }
    });
    if kept.is_some_and(|kept| kept > serial) {
        return Err(format!(
            "{server} served an older index of native mods than before"
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let partial = dir.join(".partial");
    std::fs::write(&partial, json).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, &file).map_err(|error| error.to_string())
}

/// The index the server `server` served last, kept.
pub fn server_index(data_dir: &Path, server: &str) -> Option<Index> {
    let bytes = std::fs::read(server_dir(data_dir, server).join(SERVER_INDEX)).ok()?;
    Index::parse(&bytes).ok()
}

/// The mods proposed for the server's registry, as it served them last.
pub fn server_proposals(data_dir: &Path, server: &str) -> Option<Proposals> {
    let bytes = std::fs::read(server_dir(data_dir, server).join(SERVER_PROPOSED)).ok()?;
    Proposals::parse(&bytes).ok()
}

/// The indexes to install from, the server's first.
fn indexes(
    store: &mut Store,
    data_dir: &Path,
    setup: Option<&NativeSetup>,
    server: Option<&str>,
) -> (Vec<Index>, Option<String>) {
    let mut found = Vec::new();
    let mut problem = None;
    if let Some(index) = server.and_then(|server| server_index(data_dir, server)) {
        found.push(index);
    }
    if let Some(setup) = setup.filter(|setup| !setup.keys.is_empty()) {
        match store.cached_index(&setup.keys) {
            Ok(index) => found.push(index),
            Err(StoreError::NoIndex) => {}
            Err(error) => problem = Some(format!("native mods: {error}")),
        }
    }
    (found, problem)
}

/// The panel for a game whose executable hashes to `build`, if known,
/// played on `server`, with the proposed mods when `proposed`.
pub fn view(
    data_dir: &Path,
    setup: Option<&NativeSetup>,
    build: Option<&str>,
    server: Option<&str>,
    proposed: bool,
) -> NativeView {
    let server_has = server.and_then(|server| server_index(data_dir, server));
    let keyed = setup.is_some_and(|setup| !setup.keys.is_empty());
    if server_has.is_none() && !keyed {
        return NativeView::default();
    }
    let mut view = NativeView {
        available: true,
        server: server_has.as_ref().and(server).map(str::to_owned),
        proposed_shown: proposed,
        ..NativeView::default()
    };
    let mut store = match store(data_dir) {
        Ok(store) => store,
        Err(error) => {
            view.problem = Some(error);
            return view;
        }
    };
    let (indexes, problem) = indexes(&mut store, data_dir, setup, server);
    view.problem = problem;
    view.serial = indexes.first().map(|index| index.serial);
    let proposals = proposed
        .then(|| server.and_then(|server| server_proposals(data_dir, server)))
        .flatten();
    let mut ids: Vec<&str> = store
        .registry()
        .packages
        .keys()
        .map(String::as_str)
        .collect();
    for index in &indexes {
        ids.extend(index.packages.iter().map(|p| p.id.as_str()));
    }
    if let Some(proposals) = &proposals {
        ids.extend(proposals.packages.iter().map(|p| p.id.as_str()));
    }
    ids.sort_unstable();
    ids.dedup();
    view.packages = ids
        .into_iter()
        .filter_map(|id| row(&store, &indexes, proposals.as_ref(), id, build))
        .collect();
    view
}

fn row(
    store: &Store,
    indexes: &[Index],
    proposals: Option<&Proposals>,
    id: &str,
    build: Option<&str>,
) -> Option<NativeRow> {
    let installed = store.registry().packages.get(id);
    let mut versions = indexes
        .iter()
        .map(|index| index.versions(id))
        .find(|versions| !versions.is_empty())
        .unwrap_or_default();
    let proposal = match proposals {
        Some(proposals) if versions.is_empty() => {
            versions = proposals.packages.iter().filter(|p| p.id == id).collect();
            versions.sort_by_key(|p| std::cmp::Reverse(p.semver()));
            proposals.pulls.get(id).filter(|_| !versions.is_empty())
        }
        _ => None,
    };
    let available = versions
        .iter()
        .find(|p| build.is_none_or(|build| p.runs_on(build)))
        .copied();
    let shown = installed.and_then(|i| i.package()).or(available)?;
    let settings = installed
        .and_then(|i| i.package().map(|p| (i, p)))
        .map(|(i, package)| {
            package
                .settings
                .iter()
                .map(|(key, default)| NativeSettingRow {
                    key: key.clone(),
                    value: i.settings.get(key).unwrap_or(default).clone(),
                    default: default.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(NativeRow {
        id: id.to_owned(),
        name: shown.name.clone(),
        description: shown.description.clone(),
        homepage: shown.homepage.clone(),
        simulation: changes_simulation(shown),
        available: available.map(|p| p.version.clone()),
        installed: installed.map(|i| i.current.clone()),
        previous: installed.and_then(|i| i.previous.clone()),
        enabled: installed.is_some_and(|i| i.enabled),
        listed: !versions.is_empty(),
        settings,
        proposed: proposal.cloned(),
    })
}

/// What the player does with a native mod in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Fetch the signed index again.
    Refresh,
    /// Install the newest version for this build, or upgrade to it.
    Install(String),
    /// Install a mod only proposed for the server's registry, which the
    /// player chose knowing nobody reviewed it.
    InstallProposed(String),
    Uninstall(String),
    Rollback(String),
    Enable(String, bool),
    Set(String, String, Setting),
    Reset(String),
}

/// Carries out `change`, for a game whose executable hashes to `build`.
/// Blocking: downloads run here.
pub fn apply(
    data_dir: &Path,
    setup: &NativeSetup,
    build: Option<&str>,
    server: Option<&str>,
    change: Change,
) -> Result<(), String> {
    let error = |error: StoreError| format!("native mods: {error}");
    let mut store = store(data_dir)?;
    let proposed = matches!(change, Change::InstallProposed(_));
    match change {
        // The server's index is fetched over its connection, by the caller.
        Change::Refresh if setup.keys.is_empty() => Ok(()),
        Change::Refresh => store
            .refresh(setup.source.as_ref(), &setup.keys)
            .map(drop)
            .map_err(error),
        Change::Install(id) | Change::InstallProposed(id) => {
            let build =
                build.ok_or("start the game once, or name it, so the launcher knows its build")?;
            let index = if proposed {
                server
                    .and_then(|server| server_proposals(data_dir, server))
                    .map(|proposals| proposals.index())
                    .ok_or("the server proposes no such mod now")?
            } else {
                index_listing(&mut store, data_dir, setup, server, &[(&id, None)])?
            };
            let done = store
                .install(
                    resolve::Context {
                        index: &index,
                        build,
                        registry: BUILT_IN,
                    },
                    &id,
                    setup.source.as_ref(),
                )
                .map_err(error)?;
            info!(installed = ?done, "installed native mods");
            Ok(())
        }
        Change::Uninstall(id) => store.uninstall(&id).map_err(error),
        Change::Rollback(id) => store.rollback(&id).map_err(error),
        Change::Enable(id, on) => store.set_enabled(&id, on).map_err(error),
        Change::Set(id, key, value) => store.set_setting(&id, &key, value).map_err(error),
        Change::Reset(id) => store.reset_settings(&id).map_err(error),
    }
}

/// The index with every one of `wanted` in it: the server's, else the
/// signed one kept, else a signed one fetched again.
fn index_listing(
    store: &mut Store,
    data_dir: &Path,
    setup: &NativeSetup,
    server: Option<&str>,
    wanted: &[(&str, Option<&str>)],
) -> Result<Index, String> {
    let lists = |index: &Index| {
        wanted.iter().all(|(id, version)| match version {
            Some(version) => index.package(id, version).is_some(),
            None => !index.versions(id).is_empty(),
        })
    };
    let (indexes, _) = indexes(store, data_dir, Some(setup), server);
    if let Some(index) = indexes.into_iter().find(|index| lists(index)) {
        return Ok(index);
    }
    if setup.keys.is_empty() {
        return Err("no index of native mods lists the room's".into());
    }
    store
        .refresh(setup.source.as_ref(), &setup.keys)
        .map_err(|error| format!("native mods: {error}"))
}

/// Makes the next game on the build `build` run exactly the room's native
/// mods `room`: each installed in the room's version from the signed index,
/// with the room's settings, and enabled; every other simulation-changing
/// one switched off. Ones that change only what a player sees stay the
/// player's. Returns whether anything changed, and so whether a running
/// game must start again to run them. Refused when the index does not list
/// the room's version for this build, or the result does not match the
/// room: then that player cannot join.
pub fn match_room(
    data_dir: &Path,
    setup: &NativeSetup,
    build: &str,
    server: Option<&str>,
    room: &[NativeMod],
) -> Result<bool, String> {
    let error = |error: StoreError| format!("native mods: {error}");
    let mut store = store(data_dir)?;
    let before = store.registry().clone();
    let wanted: Vec<(&str, Option<&str>)> = room
        .iter()
        .map(|m| (m.id.as_str(), Some(m.package_version())))
        .collect();
    if !room.is_empty() {
        let index = index_listing(&mut store, data_dir, setup, server, &wanted)?;
        for native in room {
            let (id, version) = (native.id.as_str(), native.package_version());
            let current = store
                .registry()
                .packages
                .get(id)
                .is_some_and(|i| i.current == version);
            if !current {
                let package = index
                    .package(id, version)
                    .filter(|p| p.runs_on(build))
                    .ok_or_else(|| {
                        format!(
                            "the signed index has no {} {version} for your game's build",
                            native.name
                        )
                    })?;
                info!(%id, %version, "installing the room's native mod");
                store
                    .install_exact(
                        resolve::Context {
                            index: &index,
                            build,
                            registry: BUILT_IN,
                        },
                        &package.id,
                        version,
                        setup.source.as_ref(),
                    )
                    .map_err(error)?;
            }
            // The room's settings, all of them: a told setting overrides the
            // default, and none of this player's own stays.
            store.reset_settings(id).map_err(error)?;
            for setting in &native.settings {
                store
                    .set_setting(id, setting.key.as_str(), from_value(&setting.value))
                    .map_err(error)?;
            }
            store.set_enabled(id, true).map_err(error)?;
        }
    }
    // Another simulation-changing mod would part this game from the room.
    let others: Vec<String> = store
        .registry()
        .packages
        .iter()
        .filter(|(id, installed)| {
            installed.enabled
                && !room.iter().any(|m| m.id.as_str() == id.as_str())
                && installed.package().is_some_and(changes_simulation)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in others {
        info!(%id, "switching off a native mod the room does not run");
        store.set_enabled(&id, false).map_err(error)?;
    }
    let (list, _) = store.enabled_for(build).map_err(error)?;
    let ours = room_terms(&store, &list)?;
    let theirs: BTreeMap<&str, &str> = room
        .iter()
        .map(|m| (m.id.as_str(), m.version.as_str()))
        .collect();
    let mine: BTreeMap<&str, &str> = ours
        .iter()
        .map(|m| (m.id.as_str(), m.version.as_str()))
        .collect();
    if theirs != mine {
        return Err(
            "your native mods still differ from the room's after installing them: the room's settings could not be matched"
                .into(),
        );
    }
    Ok(store.registry() != &before)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tpf3mp_nativemods::store::REGISTRY;

    #[test]
    fn nothing_installed_or_enabled_passes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"not really a game").unwrap();
        assert_eq!(game_env(dir.path(), &exe).unwrap(), GameNative::default());

        // Installed, not enabled.
        let root = dir.path().join(FOLDER);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(REGISTRY),
            br#"{"format":1,"serial":3,"packages":{}}"#,
        )
        .unwrap();
        assert_eq!(game_env(dir.path(), &exe).unwrap(), GameNative::default());

        // Enabled for this build: the game is told where the list is.
        let package = tpf3mp_nativemods::index::Package {
            id: "pages".into(),
            version: "1.0.0".into(),
            name: "Pages".into(),
            description: String::new(),
            homepage: None,
            simulation: false,
            builds: vec![sha256_of_file(&exe).unwrap()],
            features: Vec::new(),
            settings: Default::default(),
            depends: Vec::new(),
            conflicts: Vec::new(),
            files: Vec::new(),
            plugins: Vec::new(),
        };
        let registry = tpf3mp_nativemods::store::Registry {
            format: 1,
            serial: 3,
            index_sha256: None,
            packages: [(
                "pages".to_owned(),
                tpf3mp_nativemods::store::Installed {
                    current: "1.0.0".into(),
                    previous: None,
                    enabled: true,
                    settings: Default::default(),
                    versions: [("1.0.0".to_owned(), package)].into(),
                },
            )]
            .into(),
        };
        fs::write(root.join(REGISTRY), serde_json::to_vec(&registry).unwrap()).unwrap();
        let (name, path) = game_env(dir.path(), &exe).unwrap().env.unwrap();
        assert_eq!(name, enabled::ENV);
        let list = enabled::read(Path::new(&path)).unwrap();
        assert_eq!(list.packages[0].id, "pages");

        // Another build of the game: the list is empty.
        fs::write(&exe, b"another build").unwrap();
        let (_, path) = game_env(dir.path(), &exe).unwrap().env.unwrap();
        assert!(enabled::read(Path::new(&path)).unwrap().packages.is_empty());

        // A registry that cannot be read stops the start.
        fs::write(root.join(REGISTRY), b"{ broken").unwrap();
        assert!(game_env(dir.path(), &exe).is_err());
    }

    #[test]
    fn an_enabled_simulation_mod_for_another_build_stops_the_game_start() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"the new build").unwrap();
        let root = dir.path().join(FOLDER);
        fs::create_dir_all(&root).unwrap();
        let package = tpf3mp_nativemods::index::Package {
            id: "bigmap".into(),
            version: "1.0.0".into(),
            name: "Big Maps".into(),
            description: String::new(),
            homepage: None,
            simulation: true,
            builds: vec!["0".repeat(64)],
            features: Vec::new(),
            settings: Default::default(),
            depends: Vec::new(),
            conflicts: Vec::new(),
            files: Vec::new(),
            plugins: Vec::new(),
        };
        let registry = tpf3mp_nativemods::store::Registry {
            format: 1,
            serial: 3,
            index_sha256: None,
            packages: [(
                "bigmap".to_owned(),
                tpf3mp_nativemods::store::Installed {
                    current: "1.0.0".into(),
                    previous: None,
                    enabled: true,
                    settings: Default::default(),
                    versions: [("1.0.0".to_owned(), package)].into(),
                },
            )]
            .into(),
        };
        fs::write(root.join(REGISTRY), serde_json::to_vec(&registry).unwrap()).unwrap();

        assert!(game_env(dir.path(), &exe).is_err());
        assert!(!root.join(enabled::FILE).exists());
    }

    /// A signed index and its files, held in memory, signed with a TEST
    /// key made for the test.
    struct Fake {
        json: Vec<u8>,
        signature: Vec<u8>,
        files: BTreeMap<String, Vec<u8>>,
    }

    impl Source for Fake {
        fn index(&self) -> Result<(Vec<u8>, Vec<u8>), tpf3mp_nativemods::fetch::FetchError> {
            Ok((self.json.clone(), self.signature.clone()))
        }

        fn download(
            &self,
            file: &tpf3mp_nativemods::index::FileEntry,
            path: &Path,
        ) -> Result<(), tpf3mp_nativemods::fetch::FetchError> {
            let bytes = self.files.get(&file.url).cloned().unwrap_or_default();
            tpf3mp_nativemods::fetch::copy_verified(
                &bytes[..],
                path,
                file.size,
                &file.sha256,
                |_| {},
            )
        }
    }

    fn native_package(id: &str, version: &str, build: &str) -> (Package, (String, Vec<u8>)) {
        let bytes = format!("{id} {version}").into_bytes();
        let url = format!("https://example.invalid/{id}/{version}/data.txt");
        let package = Package {
            id: id.into(),
            version: version.into(),
            name: format!("{id} mod"),
            description: String::new(),
            homepage: Some(format!("https://example.invalid/{id}")),
            simulation: true,
            builds: vec![build.into()],
            features: Vec::new(),
            settings: BTreeMap::from([
                ("depth".to_owned(), Setting::Int(10)),
                ("raster".to_owned(), Setting::Bool(true)),
            ]),
            depends: Vec::new(),
            conflicts: Vec::new(),
            files: vec![tpf3mp_nativemods::index::FileEntry {
                path: "data.txt".into(),
                url: url.clone(),
                size: bytes.len() as u64,
                sha256: tpf3mp_nativemods::signed::sha256_hex(&bytes),
            }],
            plugins: Vec::new(),
        };
        (package, (url, bytes))
    }

    fn setup(packages: Vec<(Package, (String, Vec<u8>))>) -> NativeSetup {
        use ring::signature::{Ed25519KeyPair, KeyPair};
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let index = Index {
            format: 1,
            serial: 1,
            packages: packages.iter().map(|(p, _)| p.clone()).collect(),
        };
        let json = serde_json::to_vec(&index).unwrap();
        let signature = pair.sign(&json).as_ref().to_vec();
        NativeSetup {
            source: Arc::new(Fake {
                json,
                signature,
                files: packages.into_iter().map(|(_, file)| file).collect(),
            }),
            keys: vec![pair.public_key().as_ref().to_vec()],
        }
    }

    /// A member joining a room gets the room's version from the signed
    /// index, with the room's settings, and leaves off a simulation mod the
    /// room does not run; then their terms are the room's.
    #[test]
    fn a_member_gets_the_rooms_native_mods_from_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"the game").unwrap();
        let build = build_of(&exe).unwrap();
        let setup = setup(vec![
            native_package("bigmap", "0.3.0", &build),
            native_package("bigmap", "0.4.0", &build),
            native_package("other", "1.0.0", &build),
        ]);

        // The owner: Big Maps 0.3.0, deeper than the default.
        let owner = dir.path().join("owner");
        apply(&owner, &setup, Some(&build), None, Change::Refresh).unwrap();
        let mut store = Store::open(owner.join(FOLDER)).unwrap();
        let index = store.cached_index(&setup.keys).unwrap();
        let context = resolve::Context {
            index: &index,
            build: &build,
            registry: BUILT_IN,
        };
        store
            .install_exact(context, "bigmap", "0.3.0", setup.source.as_ref())
            .unwrap();
        store.set_enabled("bigmap", true).unwrap();
        store
            .set_setting("bigmap", "depth", Setting::Int(12))
            .unwrap();
        let room = next_terms(&owner, &exe).unwrap();
        assert_eq!(room.len(), 1);
        assert_eq!(room[0].package_version(), "0.3.0");
        assert_eq!(room[0].name.as_str(), "bigmap mod");

        // A member with the newest version and another simulation mod.
        let member = dir.path().join("member");
        apply(
            &member,
            &setup,
            Some(&build),
            None,
            Change::Install("bigmap".into()),
        )
        .unwrap();
        apply(
            &member,
            &setup,
            Some(&build),
            None,
            Change::Install("other".into()),
        )
        .unwrap();
        apply(
            &member,
            &setup,
            Some(&build),
            None,
            Change::Enable("other".into(), true),
        )
        .unwrap();
        assert_ne!(next_terms(&member, &exe).unwrap(), room);

        assert!(match_room(&member, &setup, &build, None, &room).unwrap());
        let theirs = next_terms(&member, &exe).unwrap();
        assert_eq!(theirs, room, "the same version, settings and nothing else");
        let row = view(&member, Some(&setup), Some(&build), None, false)
            .packages
            .into_iter()
            .find(|row| row.id == "other")
            .unwrap();
        assert!(!row.enabled, "the room does not run it");
        // Matched already: nothing changes, and the game need not restart.
        assert!(!match_room(&member, &setup, &build, None, &room).unwrap());

        // A version the index does not have for this build: refused.
        let mut gone = room.clone();
        gone[0].version = Text::lossy("9.9.9+0000000000");
        assert!(match_room(&member, &setup, &build, None, &gone).is_err());

        // A room without native mods: the member's are switched off.
        assert!(match_room(&member, &setup, &build, None, &[]).unwrap());
        assert!(next_terms(&member, &exe).unwrap().is_empty());
    }

    #[test]
    fn settings_travel_as_the_room_tells_them() {
        for setting in [
            Setting::Bool(true),
            Setting::Int(-7),
            Setting::Text("north".into()),
        ] {
            assert_eq!(from_value(&to_value(&setting).unwrap()), setting);
        }
        assert!(to_value(&Setting::Text("x".repeat(200))).is_err());
    }

    #[test]
    fn without_the_key_the_panel_offers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            view(dir.path(), None, None, None, false),
            NativeView::default()
        );
        let mut keyless = setup(Vec::new());
        keyless.keys.clear();
        assert!(!view(dir.path(), Some(&keyless), None, None, false).available);
        let shown = view(dir.path(), Some(&setup(Vec::new())), None, None, false);
        assert!(shown.available);
        assert_eq!(shown.serial, None, "no index fetched yet");
    }

    /// A server's own index needs no key: the launcher takes it from the
    /// server, installs the room's mods from it, and shows the mods its
    /// registry's open pull requests propose only when asked.
    #[test]
    fn a_servers_own_index_and_its_proposals() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"the game build two").unwrap();
        let build = build_of(&exe).unwrap();
        let big = native_package("bigmap", "0.3.0", &build);
        let trees = native_package("trees", "1.0.0", &build);
        let mut keyless = setup(vec![big.clone(), trees.clone()]);
        keyless.keys.clear();
        let server = "eu.example:29470";
        let index = Index {
            format: 1,
            serial: 5,
            packages: vec![big.0.clone()],
        };
        let json = serde_json::to_vec(&index).unwrap();
        let player = dir.path().join("player");
        assert!(!view(&player, Some(&keyless), Some(&build), Some(server), false).available);
        accept_server_index(&player, server, 5, &json, false).unwrap();
        // Another serial than it says, or an older index: refused.
        assert!(accept_server_index(&player, server, 6, &json, false).is_err());
        let older = Index {
            serial: 4,
            ..index.clone()
        };
        assert!(
            accept_server_index(
                &player,
                server,
                4,
                &serde_json::to_vec(&older).unwrap(),
                false
            )
            .is_err()
        );

        let shown = view(&player, Some(&keyless), Some(&build), Some(server), false);
        assert!(shown.available);
        assert_eq!(shown.server.as_deref(), Some(server));
        assert_eq!(shown.packages.len(), 1);
        // Another server's index is not this one's.
        assert!(
            !view(
                &player,
                Some(&keyless),
                Some(&build),
                Some("other:1"),
                false
            )
            .available
        );

        // A room's mod, from the server's index.
        // The digest is the defaults': match it as the owner would.
        let owner = dir.path().join("owner");
        accept_server_index(&owner, server, 5, &json, false).unwrap();
        apply(
            &owner,
            &keyless,
            Some(&build),
            Some(server),
            Change::Install("bigmap".into()),
        )
        .unwrap();
        apply(
            &owner,
            &keyless,
            Some(&build),
            Some(server),
            Change::Enable("bigmap".into(), true),
        )
        .unwrap();
        let owners = next_terms(&owner, &exe).unwrap();
        assert_eq!(owners[0].package_version(), "0.3.0");
        assert!(match_room(&player, &keyless, &build, Some(server), &owners).unwrap());
        assert_eq!(next_terms(&player, &exe).unwrap(), owners);

        // Proposals: shown only when asked, marked with their pull request.
        let proposals = Proposals {
            format: 1,
            serial: 5,
            packages: vec![trees.0.clone()],
            pulls: [(
                "trees".to_owned(),
                Proposal {
                    number: 12,
                    title: "Add Trees".into(),
                    url: "https://github.com/r/reg/pull/12".into(),
                },
            )]
            .into(),
        };
        accept_server_index(
            &player,
            server,
            5,
            &serde_json::to_vec(&proposals).unwrap(),
            true,
        )
        .unwrap();
        let hidden = view(&player, Some(&keyless), Some(&build), Some(server), false);
        assert!(hidden.packages.iter().all(|row| row.id != "trees"));
        let shown = view(&player, Some(&keyless), Some(&build), Some(server), true);
        let row = shown.packages.iter().find(|row| row.id == "trees").unwrap();
        assert_eq!(row.proposed.as_ref().unwrap().number, 12);
        // Never from the index proper; only when the player chooses it.
        assert!(
            apply(
                &player,
                &keyless,
                Some(&build),
                Some(server),
                Change::Install("trees".into())
            )
            .is_err()
        );
        apply(
            &player,
            &keyless,
            Some(&build),
            Some(server),
            Change::InstallProposed("trees".into()),
        )
        .unwrap();
    }
}
