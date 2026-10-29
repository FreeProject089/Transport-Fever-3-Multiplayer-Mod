# Performance audit, 2026-10-29

Status: five fixes and one measurement line, each checked in two real
games on one PC; the save and the industry connection are shorter, not
gone. Source: dev
`cc22098b` (v1.3.4), branch `perf/stutter-audit`. Game: Steam Windows build
40425 (EXE SHA-256 `e9a6190f…74cd`). Save: Bubu's `bubujujuoct10` (150 MB,
66 vehicles, 25 lines), the save of the 2026-10-28 pacing experiments. Two
games on this PC (Ryzen 9 5900X, 24 threads) through `tools/game/room.ps1`
with the rig's local server; no production server.

## Method: a sampling profile of the real game

`wpr` needs an elevated shell, so the audit used a small user-mode sampler
(kept outside the repository): it suspends each busy thread of one of our
test games about 1,000 times a second, reads its registers and 64 KiB of
stack, resumes it, and unwinds the stack with each module's `.pdata`.
Addresses in the game are named with `tpfre` (a fresh index of the 40425
EXE: RTTI names and inferred source files), the hook's with its PDB.
"Busy" means the thread spent at least about half the interval's cycles
(`QueryThreadCycleTime`); the engine spin-waits for its pools, so busy
samples include waits, and the shares below are read as wall time of the
thread, not CPU work.

## Findings

### 1. The GUI's `getPlayer` cost 203 µs a call (fixed)

The main thread (frame rate) spent 6.4% of all its samples, and 8.4% of
its busy ones, in `getPlayer`'s Lua binding, every sample of it inside the
hook's GUI player fix (`guiplayer.rs`, `push_body` → `closure_is_gui`) and
there in a system call. `closure_is_gui` checked the closure with the cold
`image::readable`, one `VirtualQuery` per check, four a call. In a game
process this size one `VirtualQuery` costs about 50 µs, not the
microsecond of a small process. Measured in the game's console:
20,000 calls of `api.engine.util.getPlayer()` took 203 µs each (and froze
the console long enough for the engine's hang warning).

Every per-call, per-frame or per-update check now goes through
`image::readable_cached` (the guarded check, 1-2 ns, the same answer): the
GUI fixes (getPlayer closure, owner and icon-owner sites, line viewers),
the tool player, the tick counters, the module and stop-tool reads, the
terrain writes, plugin reads and the industry probe. Install-time code
checks keep `VirtualQuery`. Test:
`guiplayer::tests::the_guis_per_call_reads_never_ask_the_system` counts
the thread's system queries (fails on the old code).

After: 0.15 µs a call (about 1,350 times faster); the binding fell from
6.37% to 0.02% of the main thread's samples.

### 2. A room save freezes every game for 6-7 s (default changed)

Periodic room saves of the 150 MB world took 6,096-7,058 ms (the game's
own `Saving game complete` line) with the game's settings. The profile of
one save (sim thread, 7.5 s): 42% zstd compression itself, 25% `std::
streambuf` (`xsputn`, `_Lock`/`_Unlock` in MSVCP140), 10% `memcpy`, 14% in
ntdll: the game streams the save through zstd level 3 with a **128-byte**
buffer, so it locks and enters the compressor every 128 bytes. The hook's
verified rewrite (level 1, 64 KiB buffer; `savefast.rs`, measured 4.4-4.6 s
on 2026-10-28) addresses both parts and is now **on by default**
(`TPF3MP_HOOK_SAVE_FAST=0` keeps the game's). Every player of a room saves
at the same step, so this is the longest freeze a room has.

After, same world: periodic saves of 4,394 and 5,103 ms and a start save
of 3,727 ms (168 MB files against 157 MB).

### 3. A command waited up to 100 ms for the server's tick (fixed)

The sequencer appended a player's command and sealed it only at its next
100 ms tick. It now seals as soon as it orders a command
(`Room::advance_and_seal`), which takes up to 100 ms (50 ms on average) off
what every player feels on their own commands. Saves still seal alone.
Test: `turns::a_command_is_sealed_when_it_arrives_not_at_the_next_tick`
(with a 4 s tick the old code took 4.0 s). In the real game (rig rebuilt
with the change): the guest renamed a line and renamed it back through the
Line Manager; each `EditLine` was handed to the room and applied in both
games within the same second, and all 17 shared checkpoints agreed.

### 4. The getComponent wrapper of the GUI (fixed, small)

`follow.loans` wraps the GUI state's `api.engine.getComponent`, which the
game's windows call many times a frame. Every call made a closure, entered
a `pcall` and read `api.type.ComponentType.GAME_SCRIPT` through the api's
bindings, whatever the kind; that roughly doubled a call (0.35 µs of 0.45
µs measured in the console). It now reads the kind once and sends any
other kind straight to the game's. Test: the finance-window test counts
the kind's reads.

### 5. Half of each simulation update is game scripts (measured, small cuts)

Inside `GameSim::Step` the simulation thread had `ecs::GameScriptSystem`'s
update on its stack 48.6% of the time, mostly waiting for script calls on
the pool, 14.8% running Lua itself. Our own script's rolling world check
costs 2.0-2.4 ms an update (`rolling-check-cost`), plus about 0.23 ms per
decode of its state (measured in the console; `update` decodes it once,
`postUpdate` once more and encodes it back). A new `script-cost:` line
gives our script's `update` and `postUpdate` time per 250 room updates in
each Lua state: on this world at 4x about 0.3-0.5 ms and 2.4-2.9 ms an
update (single postUpdates up to 20 ms), about a tenth of the step. The
rest belongs to the game's own scripts. Two exact cuts in the rolling
check's dynamic lanes: the people lane counts the engine's list instead of
copying 11,000 ids into a second table, and the economy lane hashes the
subsidy rows with the hook's hash (the same digest as the Lua one).

### 6. A new industry froze every game for about a second (halved)

Each time an industry spawns, the game connects it to the road network on
the simulation thread (`game/urbansim/init_streets_util.cpp`, `sub_905560`,
called from `sub_158d00`): `Connect industry: 850-1075 ms` in the game's
console, eight times in about 25 minutes of this world at 4x. A profile of
four of them: 61% self time in one 78-byte function, an anonymous
`BitmapRasterizable`'s span fill (`0x91a560`, vtable slot 1), which sets
`count` bits of a `std::vector<uint32_t>` one at a time (load, `bts`,
store). The hook now jumps from its entry to a fill of whole words that
leaves the same bits (`fastbitmap.rs`, optional 40425 target whose
signature is the whole function; any span below bit 0 goes to the game's
loop). Tests run the game's 78 bytes beside it on 20,000 random spans.

After: 409-466 ms in nine calls, both games' rolling checks equal. What
remains is the rasterizer's scanline walk (`sub_2b755e0`, float math, 27%),
the street pathfinder (15%), the tesselator (11%) and allocation; none is a
small exact loop like the span fill.

### 7. Long native calls that remain

- `Connect industry`, about 0.4 s (6 above).
- The save, 4.4-5.1 s (2 above).

### Step time

The game's step per update over every 4x window (`perf:` lines, windows
of 150 updates or more, saves and connections included), median of p1 / p2:
baseline 34.2 / 32.4 ms, with fixes 1-5 27.1 / 29.5 ms, with 6 too 25.8 /
28.6 ms. The runs differ in length and in the world's stage (the baseline
ran 15 minutes at 4x, the last 4), so this is observed, not a controlled
comparison.

### Other measurements

- The hook's own per-update work stays near 1% of the game's step
  (`perf:` lines); the fused emission grid and fast component lookup are in.
- Emitter splats (`EmissionEmitterSystem`, `loopresults.h` /
  `emission_util.cpp`) are about 18% of the simulation pools' work,
  `AnimalMoveSystem` about 9%.
- The main thread spends about 30% of its time in the game's React UI
  (Lua), which is the game's.
- Rolling-v2 native parts are not cheaper per update (3.5 vs 1.7 ms in the
  earlier measurement), so the rolling check was left as it is.

## The worlds stayed equal

Every rolling-check signature both games logged at the same steps was
equal in every run: 457 shared checkpoints in the baseline, 289 with fixes
1-5, 232 with the bitmap fix as well, 17 in the command check. No
divergence, resync or held world was logged. Raw logs are in
`target/game-runs/perf-{base,fix1,fix2,fix3}-bubu` on the development PC.

## Not changed, for the owner

- **Joiners and old snapshots.** A player who joins gets the room's
  current snapshot at once, however old (up to ten minutes of play), and
  replays from it at about 28 updates a second against the room's 20 at 4x:
  the nine-minute rejoin of 2026-10-28. Handing out a fresh save instead
  would make every other member reload it too, because a game that saved a
  world numbers its entities differently from one that loaded it
  (PROTOCOL.md, "Snapshots"). That trade (one long replay against everyone
  reloading) is a decision for the owner.
