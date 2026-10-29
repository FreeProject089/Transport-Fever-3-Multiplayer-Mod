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

## Micro-stutter: vehicles moving unevenly up close

The owner's report: constant micro-stutter when zoomed in on a train. The
fixes above remove freezes, not this.

**Measured.** A new `motion:` line (docs/HOOKS.md, "What the renderer
shows") works out each frame's on-screen simulation time from `totalTime`,
`lastSyncTime`, `guiFrameTime` and the batch's updates, and its rate against
the room's pace. In steady play, two games of Bubu's world on this PC:

- no frame stood still for a late or an empty batch (they occur only
  around a speed change, an industry connection or a save);
- by the game's own clock the speed was exactly 1.00 at p5 and p95, at 1x
  and 4x: the batches and the trim are even;
- by the wall clock it swung between about 0.65 and 1.35 (coefficient of
  variation 0.25-0.29; 100-135 frames in 10 s whose speed differed from
  the previous frame's by more than 30%).

A dump of 2,400 frames at 1x (`TPF3MP_HOOK_MOTION=frames`) showed why: the
`dt` the game adds to `totalTime` at frame `i` is the wall time of frame
`i - 1` (correlation +1.000; +0.27 with frame `i`). With frames of 22 to 37
ms (two 4K games on one PC), each frame moves vehicles by the previous
frame's length. Smoothing that `dt` would help little (0.23 to 0.18 at
best, simulated), since the next frame's length is not predictable.

**Fixed: frame pacing with a matching clock** (`pacing.rs`). In a room's
game the hook holds every frame to the 75th percentile of the last 120
frames' own length, and gives `CGame::Step` that target as its `dt`, the
difference to the game's own repaid a tenth a frame. Measured, same world
and PC (1x / 4x, host):

| | without | 85th percentile | 75th percentile (default) |
|---|---|---|---|
| speed variation (cv) | 0.25-0.27 / 0.25-0.29 | 0.09-0.10 / 0.11-0.12 | 0.13 / 0.11-0.14 |
| frames swinging >30% per 10 s | 115-135 / 100-118 | 18-21 / 17-19 | 25-27 / 19-22 |
| speed p5-p95 | 0.65-1.40 / 0.63-1.33 | 0.84-1.11 / 0.81-1.13 | 0.76-1.15 / 0.74-1.14 |
| fps | 37-38 / 34 | 31-32 / 29-30 | 35-36 / 30-31 |

The guest's window (in the background) lost more frames at 4x (31 to
24-26 fps at the 85th, 26-28 at the 75th) for the same gain. Two games on
one PC widen the frame times; one game per PC should lose fewer.
Every shared checkpoint agreed in all four pacing runs (45, 26, 63, 45).
`TPF3MP_HOOK_FRAME_PACING=0` turns pacing and its clock off.

**Batch handovers make their frame longer.** Frames whose step hands over
a batch (`CGame::Sync`, five a second) took a median 29.6 ms against
23.5 ms for the others (the guest: 30.3 against 23.7), a regular 5 Hz
unevenness of its own. `CGame::Sync` itself is short (0.3% of the main
thread's samples, mostly the UI's per-batch terrain and model updates in
its callback); the extra time falls later in the frame. The pacing
therefore keeps a target per kind of frame: the clock knows before the
step whether it will sync and gives that frame the handover target.
Measured at 1x against one target: host 35.5 to 36-36.5 fps, speed
variation 0.133 to 0.125; guest 33.5-34 to 35.3-35.6 fps, 0.135 to
0.12-0.13, swings 36-46 to 20-25 in 10 s. At 4x the same as one target
(host 30.2-30.5 fps, 0.12-0.13). 47 shared checkpoints agreed.

**Frame rate through the hook.** After the `getPlayer` fix the hook is
1.4% of the main thread's samples. The main thread waits 45% of its
samples, mostly on the game's render jobs (`std::future`) and the GPU
driver, so on this PC (two 4K games on one GPU) the frame rate is bound by
rendering; the hook cannot raise it much further. What pacing costs is
now about 1-5% at 1x and about 10% at 4x.
