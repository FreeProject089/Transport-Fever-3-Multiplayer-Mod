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
