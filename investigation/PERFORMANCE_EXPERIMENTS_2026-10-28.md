# Follow-up performance experiments

Base: `9051dde6`, the measured industry-history/replay fixes. Experimental
branch: `codex/experiment-short-batches-20261028`. Direct work, no subagents,
existing checkout and D: build cache. Only owned games and local relay.

## Intended outcome and acceptance

Reduce player-visible pauses and variation in vehicle motion in populated
rooms without changing ordered simulation, masking drift, or losing actions.

- Compare the same Bubu fixture, camera, resolution, speed and action workload.
  Keep raw traces and distinguish startup/catch-up from steady windows.
- Compare native-call median/p95/p99/max, positive-update gaps, zero reasons,
  backlog and CPU. Shorter calls alone do not establish lower CPU cost or
  smoother rendered movement. Keep outliers; label unmatched workloads.
- Check every overlapping rolling digest, allocator sample and tick counter.
  No unexpected divergence, resync, missing sample or held world is a pass.
- Verify ordinary menu start, actions, pause/resume and rejoin before accepting
  a scheduling change for players. Fixtures establish isolated mechanics only.
- Keep experiments opt-in until demonstrated. Preserve acknowledgement gates,
  step/checkpoint ordering, and native ownership. No promotion in this task.

## Experiment A: smaller steady batches

The baseline at 4x runs about four updates per 200 ms callback, taking roughly
120-130 ms on this PC. A 100 ms target groups two updates; 50 ms groups one.
This may spread ordinary work and reduce action latency, but increases native
buffer swaps and per-batch overhead. It cannot split a single expensive native
industry operation or save. Existing trim/catch-up and ordering remain.

`TPF3MP_HOOK_BATCH_MS=50|100|200` selects the target when the verified interval
hook is active. Unset is 200. Invalid values log refusal and retain 200. A
batch has at least one update, so 1x remains about 200 ms even at a 50 ms target.
The native update's simulated duration is never changed. An executable model
checks jitter, action barriers, checkpoint cuts, a long stall and recovery
before a real-game comparison. Results are recorded below when measured.

The archived October 9 investigation already found that one-update/20 Hz
batches sometimes sustained only 17-19 Hz under recording load, due to false
Sync/readiness losses. This experiment starts at 100 ms and measures those
losses rather than assuming 50 ms is better. The current baseline differs:
production interval/trim, early replay staging and shared industry history.
The focused interval suite passes (22 tests, including the new coupled model).

## Experiment B: restore the faster-save option for 40425

Branch `codex/experiment-savefast-40425`, based on Experiment A. The existing
save-fast option had no targets in this game's bundle and therefore could not
apply. Ported the exact compressor sites: 0x32deb0 function, 0x32dec4 level
load, 0x32e024 buffer. Its sole caller is save setup 0x32cba0; decompressor
0x32e140 shares the level constant at 0x37e4ad8, which is left unchanged.
Both optional signatures/prologues pass tpfre verification against the exact
40425 EXE. No new compression implementation: credit remains silver2127's
TPF2 Big Maps and the existing TF3 port. Off by default pending measurement.

Compare the same paused world with ordinary and fast compression, check load
success and matching world checkpoints, and report save size as well as time.
Static target verification alone does not establish a speed-up.

## 200 ms baseline

`batch200-bubu`, Bubu `bubujujuoct10.sav`, two local games, default saving.
The 4x click was initially aimed at the wrong scaled coordinate (the 1x
button). A separate console read of a React module was invalid in that state
and produced a missing-recipe error; it did not inspect the live UI. Exclude
all that setup, the initial `cpu.csv`, and the 1x samples. Actual accepted
4x starts at epoch 1793202354; the selected trace is room steps 4301-9000.
No builds/tests ran during it. Both games closed normally. All shared rolling,
allocator and tick observations match through 9850, with no missing samples.

| Peer | Calls | median / p95 / p99 / max ms | positive-gap p95 / max ms | replay zeros |
|---|---:|---|---|---:|
| P1 | 1196 | 124.1 / 153.6 / 379.8 / 1410.9 | 229.2 / 1477.3 | 6 |
| P2 | 1169 | 132.4 / 168.4 / 554.0 / 1646.9 | 234.8 / 1736.4 | 5 |

The window covers 235.8 s, ~19.91 updates/s, with total detour wall time per
update 32.93/35.14 ms (not CPU time). The separate 240.8 s whole-process CPU
sample used 7.47/7.45 cores, working sets 5.86/5.92 GiB. These differ from
the earlier day's measurements; compare the follow-up candidates here, not
those earlier CPU figures. Raw evidence and analysis script are under
`D:\TPF3MP-cargo\performance-followup-20261028`.
