//! The interface between TPF3-MP's hook and a native mod's plugin
//! (proposed D29 amendment, docs/NATIVE_MODS.md, "Plugins").
//!
//! A plugin is a native library (a Windows DLL, a Linux shared object)
//! that a native mod's package carries, pinned in the index by size and
//! SHA-256. The hook loads it into the game only when the launcher enabled
//! the package, checks its SHA-256 again, and calls its one export,
//! [`ENTRY`], an [`Init`], once, while the game is still suspended at
//! start. The plugin then **attaches** to the hooks its package declares:
//!
//! - it names a declared hook (`<package id>::<name>`), never an address:
//!   the hook finds the function by the declared signature in the running
//!   build and checks its first bytes;
//! - it hands over its detour, a function with the game function's own
//!   signature and calling convention, and a slot the hook fills with
//!   what the detour calls on to: the next plugin's detour on that
//!   function, or the game's own code;
//! - the hook patches each function once, after every plugin has
//!   attached, and only if every one of them could: a plugin that fails
//!   leaves the game unpatched and out of rooms.
//!
//! Several plugins on one function run in the load order, each calling the
//! next ([`Host::attach`]); one whose package declares it takes the function
//! over (`"use": "replace"`) is the only one there. The hook refuses a
//! plugin's hook on a function the hook itself patches for multiplayer.
//!
//! Everything here is `#[repr(C)]` and `extern "C"`, so a plugin can be
//! written in any language. Strings are UTF-8, passed as a pointer and a
//! length, never NUL-terminated.

#![no_std]

use core::ffi::c_void;

/// The interface's version. A plugin built for another is not loaded.
pub const ABI: u32 = 1;

/// The name of the function every plugin exports, an [`Init`], NUL
/// terminated for the platform's symbol lookup.
pub const ENTRY: &[u8] = b"tpf3mp_plugin_init\0";

/// What [`Init`] and the host's calls return when all went well.
pub const OK: i32 = 0;

/// What the host's calls return when they did not.
pub mod error {
    /// The package declares no hook of that name.
    pub const UNDECLARED: i32 = 1;
    /// The plugin attached to that hook already.
    pub const TWICE: i32 = 2;
    /// A pointer was null, or a string not UTF-8.
    pub const BAD_ARGUMENT: i32 = 3;
    /// Too late: the game's functions are patched already.
    pub const TOO_LATE: i32 = 4;
}

/// What the hook offers a plugin while it starts. Valid only during
/// [`Init`]; a plugin keeps none of its pointers but the slots it hands to
/// [`Host::attach`].
#[repr(C)]
pub struct Host {
    /// The interface's version: [`ABI`].
    pub abi: u32,
    /// The hook's own, passed back on every call.
    pub ctx: *mut c_void,
    /// The package's folder, where its files are.
    pub root: *const u8,
    pub root_len: usize,
    /// The package's settings, as JSON: its defaults with the player's (or
    /// the room's) choices over them.
    pub settings: *const u8,
    pub settings_len: usize,
    /// Writes a line to the hook's log, after the package's name.
    pub log: unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize),
    /// Attaches `detour` to the declared hook `target`. Before the game
    /// runs the function, the hook writes to `*next` what the detour calls
    /// on to: the next plugin's detour on it, or the game's own code. The
    /// slot must stay valid while the game runs. Returns [`OK`] or an
    /// [`error`].
    pub attach: unsafe extern "C" fn(
        ctx: *mut c_void,
        target: *const u8,
        target_len: usize,
        detour: *const c_void,
        next: *mut *const c_void,
    ) -> i32,
    /// Where the declared hook `target` is in the running game, for a
    /// plugin that reads near it; null for a hook not declared.
    pub address: unsafe extern "C" fn(
        ctx: *mut c_void,
        target: *const u8,
        target_len: usize,
    ) -> *const c_void,
}

/// A plugin's [`ENTRY`]: it attaches to its hooks and returns [`OK`], or
/// anything else to refuse to run.
pub type Init = unsafe extern "C" fn(host: *const Host) -> i32;
