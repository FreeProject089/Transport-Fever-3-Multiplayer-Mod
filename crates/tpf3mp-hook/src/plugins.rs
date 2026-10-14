//! Native mods' plugins in the game (proposed D29 amendment,
//! docs/NATIVE_MODS.md, "Plugins"): the code a package carries, loaded only
//! when the launcher enabled the package, and patched into the game only
//! where the package declares it.
//!
//! At bootstrap, while the game is still suspended, and before the hook's
//! own multiplayer patches, for the plan's packages in the load order:
//!
//! 1. every declared hook is found in the running build by its signature,
//!    and its first bytes checked ([`locate`]);
//! 2. the hooks are checked against each other and against the functions
//!    the hook patches itself ([`arrange`]): a hook on one of those, or two
//!    on one function of which one takes it over, refuse;
//! 3. each plugin file is checked against the SHA-256 its package pins
//!    ([`verify`]), loaded, and started ([`start`]): it attaches its
//!    detours to its declared hooks, by name;
//! 4. each function is patched once, its plugins' detours chained in the
//!    load order, the last calling on to the game's own code ([`patch`]).
//!
//! Any step that fails stops them all before anything is patched, and keeps
//! the game out of rooms: a plugin's code changes what nobody can tell.

use std::{collections::BTreeMap, ffi::c_void, path::Path};

use tpf3mp_hookcore::pattern::{Pattern, ScanError};
use tpf3mp_nativemods::{enabled::EnabledPackage, index::HookUse};
use tpf3mp_plugin_abi as abi;

/// A declared hook, found in the running game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub package: String,
    /// `<package id>::<name>`.
    pub target: String,
    pub access: HookUse,
    /// The function's address.
    pub address: usize,
}

/// Finds `package`'s declared hooks in `code`, whose first byte is at
/// `start`, each by its signature, checking its first bytes.
pub fn locate(package: &EnabledPackage, code: &[u8], start: usize) -> Result<Vec<Found>, String> {
    package
        .hooks
        .iter()
        .map(|hook| {
            let pattern = Pattern::parse(&hook.signature)
                .map_err(|error| format!("{}: the signature: {error:?}", hook.target))?;
            let at = match pattern.find_unique(code) {
                Ok(at) => at,
                Err(ScanError::NotFound) => {
                    return Err(format!("{} is not in this build of the game", hook.target));
                }
                Err(ScanError::Ambiguous { count }) => {
                    return Err(format!(
                        "{}'s signature matches {count} places in the game",
                        hook.target
                    ));
                }
            };
            let prologue = hex(&hook.prologue)
                .ok_or_else(|| format!("{}: the prologue is not hex", hook.target))?;
            let function = i64::try_from(at)
                .ok()
                .and_then(|at| at.checked_add(hook.offset))
                .and_then(|at| usize::try_from(at).ok())
                .filter(|at| {
                    at.checked_add(prologue.len())
                        .is_some_and(|end| end <= code.len())
                })
                .ok_or_else(|| format!("{} lies outside the game's code", hook.target))?;
            if code[function..function + prologue.len()] != prologue[..] {
                return Err(format!(
                    "{} does not start with the bytes its package expects",
                    hook.target
                ));
            }
            Ok(Found {
                package: package.id.clone(),
                target: hook.target.clone(),
                access: hook.access,
                address: start + function,
            })
        })
        .collect()
}

fn hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    text.len()
        .is_multiple_of(2)
        .then(|| {
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
                .collect()
        })
        .flatten()
}

/// The found hooks by function, each function's in the load order (the
/// order of `found`), unless one is a function the hook patches itself
/// (`core`), or one function has two of which one takes it over.
pub fn arrange(found: Vec<Found>, core: &[usize]) -> Result<BTreeMap<usize, Vec<Found>>, String> {
    let mut sites: BTreeMap<usize, Vec<Found>> = BTreeMap::new();
    for hook in found {
        if core.contains(&hook.address) {
            return Err(format!(
                "{} patches a function TPF3-MP patches for multiplayer",
                hook.target
            ));
        }
        sites.entry(hook.address).or_default().push(hook);
    }
    for users in sites.values() {
        if users.len() > 1
            && let Some(taker) = users.iter().find(|hook| hook.access == HookUse::Replace)
        {
            let other = users
                .iter()
                .find(|hook| hook.target != taker.target)
                .map_or("another", |hook| hook.target.as_str());
            return Err(format!(
                "{} takes over a function {other} patches too",
                taker.target
            ));
        }
    }
    Ok(sites)
}

/// Whether the file at `path` is the bytes whose SHA-256 is `sha256`.
pub fn verify(path: &Path, sha256: &str) -> Result<(), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let actual: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    if actual.eq_ignore_ascii_case(sha256) {
        Ok(())
    } else {
        Err(format!(
            "{} is not the plugin its package pins: it changed since it was installed",
            path.display()
        ))
    }
}

/// A detour a plugin attached, and the slot it calls on through.
#[derive(Debug)]
pub struct Attachment {
    pub target: String,
    pub address: usize,
    pub detour: *const c_void,
    pub next: *mut *const c_void,
}

/// What a plugin's [`abi::Host`] calls reach while it starts.
struct Starting<'a> {
    package: &'a str,
    found: &'a [Found],
    attached: Vec<Attachment>,
    log: &'a mut dyn FnMut(&str),
}

/// Starts a plugin of `package`: calls its `init` with its found hooks, and
/// returns the detours it attached. One that returns other than
/// [`abi::OK`] refuses.
///
/// # Safety
///
/// `init` is a plugin's entry, which runs its code.
#[allow(unsafe_code)]
pub unsafe fn start(
    package: &EnabledPackage,
    found: &[Found],
    init: abi::Init,
    log: &mut dyn FnMut(&str),
) -> Result<Vec<Attachment>, String> {
    let root = package.root.to_string_lossy().into_owned();
    let settings = serde_json::to_string(&package.settings).unwrap_or_default();
    let mut starting = Starting {
        package: &package.id,
        found,
        attached: Vec::new(),
        log,
    };
    let host = abi::Host {
        abi: abi::ABI,
        ctx: (&raw mut starting).cast(),
        root: root.as_ptr(),
        root_len: root.len(),
        settings: settings.as_ptr(),
        settings_len: settings.len(),
        log: host_log,
        attach: host_attach,
        address: host_address,
    };
    // SAFETY: the caller vouches for `init`; `host` and what it points to
    // outlive the call.
    let status = unsafe { init(&raw const host) };
    if status != abi::OK {
        return Err(format!(
            "{}'s plugin refused to start ({status})",
            package.id
        ));
    }
    Ok(starting.attached)
}

/// `text`, `len` bytes of UTF-8 at `text`.
///
/// # Safety
///
/// `text` points to `len` readable bytes, or is null.
#[allow(unsafe_code)]
unsafe fn text<'a>(text: *const u8, len: usize) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the caller's promise.
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, len) }).ok()
}

#[allow(unsafe_code)]
unsafe extern "C" fn host_log(ctx: *mut c_void, line: *const u8, len: usize) {
    // SAFETY: `ctx` is the `Starting` of the plugin being started, alive
    // through its `init`.
    let starting = unsafe { &mut *ctx.cast::<Starting<'_>>() };
    // SAFETY: the plugin passes its own text.
    if let Some(line) = unsafe { text(line, len) } {
        (starting.log)(&format!("plugin {}: {line}", starting.package));
    }
}

#[allow(unsafe_code)]
unsafe extern "C" fn host_attach(
    ctx: *mut c_void,
    target: *const u8,
    target_len: usize,
    detour: *const c_void,
    next: *mut *const c_void,
) -> i32 {
    // SAFETY: as in `host_log`.
    let starting = unsafe { &mut *ctx.cast::<Starting<'_>>() };
    // SAFETY: the plugin passes its own text.
    let Some(target) = (unsafe { text(target, target_len) }) else {
        return abi::error::BAD_ARGUMENT;
    };
    if detour.is_null() || next.is_null() {
        return abi::error::BAD_ARGUMENT;
    }
    let Some(found) = starting.found.iter().find(|found| found.target == target) else {
        return abi::error::UNDECLARED;
    };
    if starting.attached.iter().any(|a| a.target == target) {
        return abi::error::TWICE;
    }
    starting.attached.push(Attachment {
        target: target.to_owned(),
        address: found.address,
        detour,
        next,
    });
    abi::OK
}

#[allow(unsafe_code)]
unsafe extern "C" fn host_address(
    ctx: *mut c_void,
    target: *const u8,
    target_len: usize,
) -> *const c_void {
    // SAFETY: as in `host_log`.
    let starting = unsafe { &*ctx.cast::<Starting<'_>>() };
    // SAFETY: the plugin passes its own text.
    unsafe { text(target, target_len) }
        .and_then(|target| starting.found.iter().find(|found| found.target == target))
        .map_or(std::ptr::null(), |found| found.address as *const c_void)
}

/// Patches each function the plugins attached to, once: the first
/// attached detour in the load order runs, each calls on to the next, the
/// last to the game's own code. All or nothing: a patch that fails takes
/// back those made.
///
/// # Safety
///
/// The game's code at each address is a function the hook found and
/// checked, not running on another thread; each attachment's detour and
/// slot are its plugin's, loaded for good.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
pub unsafe fn patch(
    attachments: Vec<Attachment>,
) -> Result<Vec<tpf3mp_hookcore::detour::InlineDetour>, String> {
    use tpf3mp_hookcore::detour::InlineDetour;
    let mut by_site: BTreeMap<usize, Vec<Attachment>> = BTreeMap::new();
    for attachment in attachments {
        by_site
            .entry(attachment.address)
            .or_default()
            .push(attachment);
    }
    let mut patched = Vec::new();
    for (address, chain) in by_site {
        for pair in chain.windows(2) {
            // SAFETY: the plugin handed this slot over for the hook to fill.
            unsafe { *pair[0].next = pair[1].detour };
        }
        // SAFETY: the caller's promise for the function at `address`.
        let detour =
            unsafe { InlineDetour::install(address as *mut u8, chain[0].detour.cast()) }
                .map_err(|error| format!("{} cannot be patched: {error:?}", chain[0].target))?;
        if let Some(last) = chain.last() {
            // SAFETY: as above; the game runs nothing until the hook is
            // done.
            unsafe { *last.next = detour.trampoline().cast() };
        }
        patched.push(detour);
    }
    Ok(patched)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tpf3mp_nativemods::index::HookDecl;

    use super::*;

    fn package(id: &str, hooks: Vec<HookDecl>) -> EnabledPackage {
        EnabledPackage {
            id: id.into(),
            version: "1.0.0".into(),
            simulation: true,
            features: Vec::new(),
            settings: BTreeMap::new(),
            root: std::path::PathBuf::from(format!("native-mods/{id}/1.0.0")),
            plugins: Vec::new(),
            hooks,
        }
    }

    fn hook(target: &str, signature: &str, prologue: &str, access: HookUse) -> HookDecl {
        HookDecl {
            target: target.into(),
            signature: signature.into(),
            offset: 0,
            prologue: prologue.into(),
            access,
        }
    }

    /// A function that returns its argument plus one, padded so a detour
    /// can take its first 14 bytes, after some other code.
    const CODE: &[u8] = &[
        0xCC, 0xCC, 0xCC, 0xCC, // padding
        0x89, 0xC8, // mov eax, ecx
        0x83, 0xC0, 0x01, // add eax, 1
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, // nops
        0xC3, // ret
    ];
    const SIGNATURE: &str = "89 C8 83 C0 01 90 90 90";

    #[test]
    fn hooks_are_found_by_signature_and_checked() {
        let found = locate(
            &package("a", vec![hook("a::add", SIGNATURE, "89C8", HookUse::Chain)]),
            CODE,
            0x1000,
        )
        .unwrap();
        assert_eq!(found[0].address, 0x1004);
        let missing = locate(
            &package(
                "a",
                vec![hook(
                    "a::add",
                    "89 C8 83 C0 02 90 90 90",
                    "89",
                    HookUse::Chain,
                )],
            ),
            CODE,
            0,
        );
        assert!(missing.unwrap_err().contains("not in this build"));
        let wrong = locate(
            &package("a", vec![hook("a::add", SIGNATURE, "C3", HookUse::Chain)]),
            CODE,
            0,
        );
        assert!(wrong.unwrap_err().contains("does not start with"));
        let vague = locate(
            &package(
                "a",
                vec![hook("a::nop", "90 90 90 90", "90", HookUse::Chain)],
            ),
            CODE,
            0,
        );
        assert!(vague.unwrap_err().contains("matches"));
    }

    #[test]
    fn functions_are_shared_by_chains_and_never_by_a_takeover() {
        let at = |package: &str, target: &str, access, address| Found {
            package: package.into(),
            target: target.into(),
            access,
            address,
        };
        let sites = arrange(
            vec![
                at("a", "a::add", HookUse::Chain, 0x10),
                at("b", "b::add", HookUse::Chain, 0x10),
                at("c", "c::tick", HookUse::Replace, 0x20),
            ],
            &[0x99],
        )
        .unwrap();
        let order: Vec<&str> = sites[&0x10].iter().map(|f| f.target.as_str()).collect();
        assert_eq!(order, ["a::add", "b::add"], "in the load order");
        let taken = arrange(
            vec![
                at("a", "a::add", HookUse::Chain, 0x10),
                at("b", "b::own", HookUse::Replace, 0x10),
            ],
            &[],
        );
        assert!(taken.unwrap_err().contains("b::own takes over"));
        let core = arrange(vec![at("a", "a::step", HookUse::Chain, 0x99)], &[0x99]);
        assert!(core.unwrap_err().contains("for multiplayer"));
    }

    #[test]
    fn a_plugin_file_must_be_the_pinned_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugin.dll");
        std::fs::write(&path, b"plugin").unwrap();
        let pinned: String = ring::digest::digest(&ring::digest::SHA256, b"plugin")
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(verify(&path, &pinned), Ok(()));
        std::fs::write(&path, b"plugin, changed").unwrap();
        assert!(verify(&path, &pinned).is_err());
    }

    // Two plugins chained on one live function, as the game would run them:
    // each adds its own, then calls on.
    #[cfg(target_arch = "x86_64")]
    mod live {
        use std::ffi::c_void;

        use super::*;

        type Add = unsafe extern "C" fn(i32) -> i32;
        static mut NEXT_TEN: *const c_void = std::ptr::null();
        static mut NEXT_HUNDRED: *const c_void = std::ptr::null();

        #[allow(unsafe_code)]
        unsafe extern "C" fn add_ten(x: i32) -> i32 {
            // SAFETY: the hook filled the slot before the function ran.
            let next: Add = unsafe { std::mem::transmute(NEXT_TEN) };
            // SAFETY: `next` has the function's own signature.
            unsafe { next(x) + 10 }
        }

        #[allow(unsafe_code)]
        unsafe extern "C" fn add_hundred(x: i32) -> i32 {
            // SAFETY: as above.
            let next: Add = unsafe { std::mem::transmute(NEXT_HUNDRED) };
            // SAFETY: as above.
            unsafe { next(x) + 100 }
        }

        fn attach(
            host: *const abi::Host,
            name: &str,
            detour: *const c_void,
            slot: *mut *const c_void,
        ) -> i32 {
            #[allow(unsafe_code)]
            // SAFETY: the host the hook passed, during init.
            unsafe {
                ((*host).attach)((*host).ctx, name.as_ptr(), name.len(), detour, slot)
            }
        }

        #[allow(unsafe_code)]
        unsafe extern "C" fn init_ten(host: *const abi::Host) -> i32 {
            assert_eq!(
                attach(host, "other::add", std::ptr::null(), std::ptr::null_mut()),
                abi::error::BAD_ARGUMENT
            );
            assert_eq!(
                attach(
                    host,
                    "ten::nothing",
                    add_ten as *const c_void,
                    &raw mut NEXT_TEN
                ),
                abi::error::UNDECLARED
            );
            attach(
                host,
                "ten::add",
                add_ten as *const c_void,
                &raw mut NEXT_TEN,
            )
        }

        #[allow(unsafe_code)]
        unsafe extern "C" fn init_hundred(host: *const abi::Host) -> i32 {
            attach(
                host,
                "hundred::add",
                add_hundred as *const c_void,
                &raw mut NEXT_HUNDRED,
            )
        }

        #[allow(unsafe_code)]
        unsafe extern "C" fn init_refusing(_: *const abi::Host) -> i32 {
            7
        }

        #[test]
        #[allow(unsafe_code)]
        fn chained_plugins_each_run_then_the_games_own_code() {
            let code = tpf3mp_hookcore::detour::executable(CODE).unwrap();
            let base = code.as_ptr() as usize;
            // SAFETY: the buffer holds `CODE`, a function at offset 4.
            let game: Add = unsafe { std::mem::transmute(base + 4) };
            // SAFETY: as above.
            assert_eq!(unsafe { game(5) }, 6);
            // SAFETY: the buffer's bytes, read only.
            let image = unsafe { std::slice::from_raw_parts(code.as_ptr(), CODE.len()) };

            let ten = package(
                "ten",
                vec![hook("ten::add", SIGNATURE, "89C8", HookUse::Chain)],
            );
            let hundred = package(
                "hundred",
                vec![hook("hundred::add", SIGNATURE, "89C8", HookUse::Chain)],
            );
            let mut lines = Vec::new();
            let mut log = |line: &str| lines.push(line.to_owned());
            let mut found = locate(&ten, image, base).unwrap();
            found.extend(locate(&hundred, image, base).unwrap());
            arrange(found.clone(), &[]).unwrap();
            let mut attached = Vec::new();
            for (package, init) in [(&ten, init_ten as abi::Init), (&hundred, init_hundred)] {
                let own: Vec<Found> = found
                    .iter()
                    .filter(|f| f.package == package.id)
                    .cloned()
                    .collect();
                // SAFETY: the inits above are this test's own.
                attached.extend(unsafe { start(package, &own, init, &mut log) }.unwrap());
            }
            // SAFETY: the function is this test's, running on no other thread.
            let detours = unsafe { patch(attached) }.unwrap();
            // SAFETY: as above; patched, it runs ten's, hundred's, then its own.
            assert_eq!(unsafe { game(5) }, 116);
            drop(detours);
            // SAFETY: as above, unpatched again.
            assert_eq!(unsafe { game(5) }, 6);

            // SAFETY: as above.
            let refused = unsafe { start(&ten, &found, init_refusing, &mut log) };
            assert!(refused.unwrap_err().contains("refused to start (7)"));
        }
    }
}
