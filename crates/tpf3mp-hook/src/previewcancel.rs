//! Withdraw a street or track preview when the player aborts the proposal
//! without closing its tool. Build 40408's IA_ABORT handler (0x58d100)
//! reaches the builder's reset (0x576330), which clears its proposal and
//! renderer. The active-tool list stays unchanged, so Lua cannot infer this
//! cancellation from that list. The reset also handles completed builds.
//!
//! Only advisory preview state changes here. The game's reset still runs
//! with its original arguments; a new proposal may publish a new preview.
//!
//! The street and track tools' continuation ([`crate::toolchain`]) also
//! hears each reset, with where it was called from: the tool's game-state
//! swap resets when the entity its proposal snaps to changed revision, as
//! the room's build landing under the cursor does, and that reset must not
//! drop the kept click the way an abort does.

#![allow(unsafe_code)]
#![cfg_attr(not(all(windows, target_arch = "x86_64")), allow(dead_code))]

use std::sync::atomic::{AtomicUsize, Ordering};

pub use crate::build_data::native::previewcancel::{RESET_TARGET, SWAP_RESET_CALL};
use crate::toolchain::ResetFrom;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// Where the state swap's reset returns to; 0 when its call site did not
/// resolve or check, and then every reset counts as one from elsewhere.
static SWAP_RETURN: AtomicUsize = AtomicUsize::new(0);
/// The game's image base, so the log names a call site as the profiles do.
static IMAGE_BASE: AtomicUsize = AtomicUsize::new(0);
type Reset = unsafe extern "C-unwind" fn(usize, u8);

/// Where the reset called from `return_address` came from.
fn reset_from(return_address: usize) -> ResetFrom {
    let swap = SWAP_RETURN.load(Ordering::Acquire);
    if swap != 0 && return_address == swap {
        ResetFrom::StateSwap
    } else {
        ResetFrom::Elsewhere
    }
}

/// The reset's detour, entered through [`reset_entry`] with where the
/// reset returns to as a third argument.
unsafe extern "C-unwind" fn reset_detour(builder: usize, all: u8, return_address: usize) {
    if crate::lua::in_room() {
        crate::previews::withdraw_network();
        // A click's own reset (right after its Add, `all` 0) and the state
        // swap's keep its continuation; any other cancels it even when the
        // tool stays open.
        crate::toolchain::proposal_reset(
            builder,
            all,
            reset_from(return_address),
            return_address.wrapping_sub(IMAGE_BASE.load(Ordering::Acquire)),
        );
    }
    // SAFETY: the profile-checked reset's trampoline, with the game's
    // original arguments. Clear first so a new proposal from the reset
    // can publish its own preview afterwards.
    unsafe {
        std::mem::transmute::<usize, Reset>(ORIGINAL.load(Ordering::Acquire))(builder, all);
    }
}

/// The reset's detour entry: passes `[rsp]`, where the reset returns to, to
/// [`reset_detour`] in `r8`, and jumps there with the stack and the two
/// argument registers as the caller left them. The reset takes two
/// arguments, so `r8` is free (volatile, and no argument of its). A tail
/// call into the reset (the abort's) shows its own caller's return.
#[cfg(all(windows, target_arch = "x86_64"))]
#[unsafe(naked)]
unsafe extern "C" fn reset_entry() {
    core::arch::naked_asm!(
        "mov r8, [rsp]",
        "jmp {detour}",
        detour = sym reset_detour,
    )
}

/// Where the state swap's reset returns to: the profile's call site, which
/// must call the reset at `reset`.
pub fn swap_return(
    at: &dyn Fn(&str) -> Result<usize, String>,
    code: &dyn Fn(usize) -> Option<[u8; 5]>,
    reset: usize,
) -> Result<usize, String> {
    let call = at(SWAP_RESET_CALL)?;
    if crate::toolchain::call_target(code, call) != Some(reset) {
        return Err(format!("{SWAP_RESET_CALL} does not call {RESET_TARGET}"));
    }
    Ok(call + 5)
}

/// Install before any game tool runs.
///
/// # Safety
///
/// `at` resolves checked targets in this game's image and `detour` keeps
/// its trampoline alive, as `InlineDetour::install` requires.
#[cfg(all(windows, target_arch = "x86_64"))]
pub unsafe fn install(
    at: &dyn Fn(&str) -> Result<usize, String>,
    detour: unsafe fn(*mut u8, *const u8) -> Result<usize, String>,
) -> String {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    // SAFETY: a null name asks for the process's own executable, which
    // stays loaded for the life of the process.
    IMAGE_BASE.store(
        unsafe { GetModuleHandleW(std::ptr::null()) } as usize,
        Ordering::Release,
    );
    let code = |address: usize| -> Option<[u8; 5]> {
        if !crate::image::readable(address, 5) {
            return None;
        }
        // SAFETY: five readable bytes of the game's mapped code, only read.
        Some(unsafe { std::ptr::read_unaligned(address as *const [u8; 5]) })
    };
    let result = at(RESET_TARGET).and_then(|address| {
        // Before the detour runs: the swap's call site, checked against the
        // reset itself; without it every reset counts as an abort.
        let swap = swap_return(at, &code, address);
        SWAP_RETURN.store(*swap.as_ref().unwrap_or(&0), Ordering::Release);
        // SAFETY: the caller's; this reset does not run during bootstrap.
        unsafe { detour(address as *mut u8, reset_entry as *const u8) }.map(|o| (o, swap))
    });
    match result {
        Ok((original, swap)) => {
            ORIGINAL.store(original, Ordering::Release);
            crate::toolchain::set_reset_hooked(true);
            let swap = match swap {
                Ok(at) => format!(
                    "the state swap's reset (returning to {at:#x}) keeps the tools' continuation"
                ),
                Err(why) => format!(
                    "every reset other than the click's own ends the tools' continuation: {why}"
                ),
            };
            format!(
                "build previews: street and track proposal resets withdraw the player's preview; \
                 {swap}"
            )
        }
        Err(why) => format!("build previews: proposal reset withdrawal unavailable: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tpf3mp_proto::{BoundedVec, Payload, Text, action::*};

    fn network(track: bool) -> Payload {
        let polyline = Polyline::new(
            BoundedVec::new(vec![
                Vertex {
                    pos: Pos { x: 0, y: 0, z: 0 },
                    resolve: Resolve::New,
                },
                Vertex {
                    pos: Pos {
                        x: 1000,
                        y: 0,
                        z: 0,
                    },
                    resolve: Resolve::New,
                },
            ])
            .unwrap(),
            BoundedVec::new(vec![Link {
                from: 0,
                to: 1,
                tangent0: Tangent { x: 1, y: 0, z: 0 },
                tangent1: Tangent { x: 1, y: 0, z: 0 },
                structure: Structure::Ground,
                kind: None,
                decorations: BoundedVec::default(),
                locked: false,
                owned: false,
                lanes: BoundedVec::default(),
                precedence: None,
                position0: None,
                position1: None,
                kept: BoundedVec::default(),
            }])
            .unwrap(),
            BoundedVec::default(),
        )
        .unwrap();
        let action = if track {
            Action::BuildTrack(TrackBuild {
                track: Text::new("track/standard.lua").unwrap(),
                style: None,
                catenary: false,
                polyline,
                crossing_type: None,
            })
        } else {
            Action::BuildRoad(RoadBuild {
                street: Text::new("street/standard.lua").unwrap(),
                style: None,
                bus_lane: false,
                tram: Tram::None,
                polyline,
                crossing_type: None,
            })
        };
        action.to_payload().unwrap()
    }

    struct Builder {
        reset: Option<u8>,
        next: Option<tpf3mp_proto::Payload>,
    }

    unsafe extern "C-unwind" fn original(builder: usize, all: u8) {
        // SAFETY: each test hands its own live Builder to the reset.
        let builder = unsafe { &mut *(builder as *mut Builder) };
        builder.reset = Some(all);
        if let Some(next) = builder.next.take() {
            crate::previews::show(Some(next));
        }
    }

    #[test]
    fn only_the_checked_swap_call_site_counts_as_the_state_swap() {
        let reset = 0x1_4057_5650_usize;
        let call = 0x1_4056_cdff_usize;
        let rel = (reset as i64 - (call as i64 + 5)) as i32;
        let mut bytes = [0xE8, 0, 0, 0, 0];
        bytes[1..].copy_from_slice(&rel.to_le_bytes());
        let at = |name: &str| {
            if name == SWAP_RESET_CALL {
                Ok(call)
            } else {
                Err(format!("{name} is not in the profile"))
            }
        };
        let code = |address: usize| (address == call).then_some(bytes);
        assert_eq!(swap_return(&at, &code, reset), Ok(call + 5));
        assert!(
            swap_return(&at, &code, reset + 0x10).is_err(),
            "calls another function"
        );
        assert!(swap_return(&|_| Err("absent".into()), &code, reset).is_err());
        assert!(swap_return(&at, &|_| None, reset).is_err(), "unreadable");
        let mut jump = bytes;
        jump[0] = 0xE9;
        assert!(
            swap_return(&at, &|_| Some(jump), reset).is_err(),
            "not a call"
        );
        // Unresolved: every reset counts as one from elsewhere.
        SWAP_RETURN.store(0, Ordering::Release);
        assert_eq!(reset_from(call + 5), ResetFrom::Elsewhere);
        assert_eq!(reset_from(0), ResetFrom::Elsewhere);
        SWAP_RETURN.store(call + 5, Ordering::Release);
        assert_eq!(reset_from(call + 5), ResetFrom::StateSwap);
        assert_eq!(reset_from(call), ResetFrom::Elsewhere);
        assert_eq!(
            reset_from(0x1_4058_935f),
            ResetFrom::Elsewhere,
            "UpdateEngine's"
        );
        assert_eq!(reset_from(0), ResetFrom::Elsewhere);
        SWAP_RETURN.store(0, Ordering::Release);
    }

    #[test]
    fn aborting_with_the_tool_still_open_withdraws_the_preview_once() {
        let _serial = crate::lua::tests::SERIAL
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::previews::clear();
        crate::lua::set_in_room(true);
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        for track in [false, true] {
            crate::previews::clear();
            let action = network(track);
            crate::previews::show(Some(action));
            let now = Instant::now();
            assert!(crate::previews::take_out(now).unwrap().is_some());
            let mut builder = Builder {
                reset: None,
                next: None,
            };
            unsafe { reset_detour((&mut builder as *mut Builder) as usize, 1, 0) };
            assert_eq!(builder.reset, Some(1), "the game's reset still ran");
            assert_eq!(
                crate::previews::take_out(now + crate::previews::MIN_INTERVAL),
                Some(None)
            );
            assert_eq!(
                crate::previews::take_out(now + crate::previews::KEEPALIVE),
                None
            );
        }
        crate::lua::set_in_room(false);
        crate::previews::clear();
    }

    #[test]
    fn reset_keeps_plain_games_and_a_new_proposal_published_by_the_game() {
        let _serial = crate::lua::tests::SERIAL
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::previews::clear();
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        let action = network(false);
        crate::lua::set_in_room(false);
        crate::previews::show(Some(action.clone()));
        let mut builder = Builder {
            reset: None,
            next: None,
        };
        unsafe { reset_detour((&mut builder as *mut Builder) as usize, 0, 0) };
        assert_eq!(builder.reset, Some(0));
        let now = Instant::now();
        assert_eq!(crate::previews::take_out(now), Some(Some(action.clone())));
        crate::lua::set_in_room(true);
        builder.next = Some(action.clone());
        unsafe { reset_detour((&mut builder as *mut Builder) as usize, 1, 0) };
        assert_eq!(
            crate::previews::take_out(now + crate::previews::MIN_INTERVAL),
            Some(Some(action))
        );
        crate::lua::set_in_room(false);
        crate::previews::clear();
    }

    #[test]
    fn an_inactive_network_builder_does_not_withdraw_a_construction_preview() {
        let _serial = crate::lua::tests::SERIAL
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::previews::clear();
        crate::lua::set_in_room(true);
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        let action = crate::lua::tests::depot_build().to_payload().unwrap();
        crate::previews::show(Some(action.clone()));
        let mut builder = Builder {
            reset: None,
            next: None,
        };
        unsafe { reset_detour((&mut builder as *mut Builder) as usize, 1, 0) };
        assert_eq!(builder.reset, Some(1));
        assert_eq!(
            crate::previews::take_out(Instant::now()),
            Some(Some(action))
        );
        crate::lua::set_in_room(false);
        crate::previews::clear();
    }
}
