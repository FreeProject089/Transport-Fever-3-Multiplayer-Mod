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

## Experiment C: bounded catch-up batches

Branch `codex/experiment-catchup-budget-20261028`, based on the preceding
experiments. `TPF3MP_HOOK_MAX_BATCH_UPDATES=4|8|16` caps each released batch;
unset remains 16, invalid values log refusal and keep 16. It limits existing
gate batches, including command drains, rather than consuming steps and then
discarding any. Checkpoint/action/save/load boundaries and replay ack remain.

At the baseline's ~33-35 ms/update, 16 updates imply ~0.53-0.56 s of work.
Four would imply ~0.13-0.14 s, before batch overhead and slow native operations.
This is a capacity estimate, not a measured gain. A 20-update/s room needs
<50 ms/update including overhead; the candidate must still clear backlog,
including rejoin, or shorter individual calls are not an acceptable result.
Single expensive operations remain indivisible. Test independently of B's
compression and compare A's 100 ms/16 limit before 100 ms/4 limit.

## 100 ms observation and comparison limitations

`batch100-bubu`, same fixture, 100 ms batches, original 16-update cap,
normal compression. Shared queue/rolling/ticks all match through 18550,
with no missing overlapping observations. Both games quit cleanly.
Steps 4301-9000: P1 median/p95/p99/max 63.6/87.4/330.6/1478.8 ms;
P2 70.2/106.4/570.2/1998.5 ms. Positive gaps max 6495.6/7673.3 ms.
8/11 replay-zero calls and two save-zero calls per peer. The periodic room
save ran at epochs 1793203745-3751; this did not fall in the baseline's
selected window. Throughput ~19.14/19.17 updates/s including that save.

**Not a controlled CPU or worst-gap comparison:** the baseline's incorrect
console-close coordinate also swallowed its host zoom. Its guest and both
100 ms games did zoom. CPU was 7.94/7.41 cores and is retained only as a
run observation. Do not infer CPU regression/improvement from these runs.
The corrected paired/crossover runs retain the fixture camera (`-NoZoom`),
use the same room, and swap variants across seats. Raw first-run evidence
is retained, not replaced by the improved methodology.

## Paired AB: identical camera and world

`paired-ab-bubu`: P1 200 ms/max16, P2 100 ms/max4, normal compression,
3840x2161 windows, exact fixture camera, P1 foreground during measurement.
DLL hash, EXE hash, source commit and settings are in the run's metadata.json.
No builds, tests, clicks or recording during steps 1001-8000.

| Peer/policy | median/p95/p99/max call ms | gap p95/max ms | updates/s | p95 backlog |
|---|---|---|---:|---:|
| P1 200/16 | 125.7/153.1/470.5/1592.3 | 232.2/1631.1 | 19.898 | 13 |
| P2 100/4 | 73.0/137.1/160.0/1704.7 | 173.0/1753.9 | 19.843 | 31 |

Native detour elapsed/update: 33.38 vs 35.60 ms. Whole-process CPU over
361.3 s: P1 7.71 cores, P2 7.38; working sets 5.83/5.70 GiB. This is still
host/guest confounded until roles are swapped. The candidate lowers ordinary
and catch-up call tails, but its larger backlog and extra readiness failures
are a concern, not evidence of a complete fix. No periodic save occurred in
this window. Six/five replay-zero calls occurred; all outliers retained.

After measurement, host pause/resume reached both games and P2 issued four
company renames at 4x. All four appeared in both logs, and P1 displayed the
last name. Final checkpoint/allocator/tick comparison is in
`paired-ab-bubu/final-comparison.json`. Both owned games quit normally.

Possible next experiment if the crossover confirms the backlog problem:
small recovery batches before the 1.5-second far-behind threshold. At a
hypothetical 35 ms/update plus 40 ms batch/copy/render overhead, two updates
need 110 ms (18.18 updates/s); three need 145 ms (20.69 updates/s). This
calculation motivates testing bounded extra work, not reducing checks or
inventing steps. Presentation interval must still reflect actual count.
No such recovery-policy change is implemented in the paired AB/BA binaries.
