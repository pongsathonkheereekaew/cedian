# The panel's thread as a virtualized list (2026-10-15)

The cedian panel drew its thread as one scrolling column holding every history line, message and tool card, and rebuilt every row's text on each frame. A scroll or a streamed token therefore laid out and painted the whole session. This change makes the column a GPUI `list`, which lays out only the rows on screen, and rebuilds the rows only when the thread, the loaded history or the subagents change (`Thread::revision`).

## What ran

- `cargo bench -p cedian_bench --bench thread_render`: GPUI's `bench_renderer` on the headless Metal renderer. Each iteration changes the view (a 24 px scroll step, or one streamed text delta), then draws and presents one frame of a 420 x 900 px panel.
- The workload is a real `Thread` fed through `Thread::apply`. Each turn has a prompt, a thinking block, a reply of about 900 characters streamed a few words at a time, and two tool cards. History is the `sessions::message_line` shape, one line per older message. The sizes come from this machine's 289 OMP sessions, which have a median of 28 messages, a p90 of 601 and a max of 4,756.
- The bench views copy the panel's element shape, `Label` per row, rather than mounting `CedianPanel`, whose workspace and project graph is too large to build in release on this disk. `ThreadBefore` is the old `render` body, `ThreadAfter` is the new list, and `cached` adds the revision check.
- The bench profile ran with LTO off and 16 codegen units, to fit the disk. The shipped release profile is thin LTO with 1 unit, so absolute times there will be somewhat lower.
- 3 runs of the whole binary, each side 10 samples. The machine's load average was 7 to 14, because another session's `cargo test` was running. For that reason the primary metric is foreground instructions per frame, which that load does not move. Wall-time draw percentiles are reported next to them, with their range across runs.
- Validation outside timing: the list drew about 6 rows per frame out of 125 to 2,002 thread rows, and 13 out of 100 to 2,000 history lines. The old column drew every row.

## Results

Foreground instructions per frame (median of 3 runs) and draw p50:

| Workload | Before | List | List + cached rows |
|---|---|---|---|
| Thread scroll, 25 turns | 15.8 M, 0.90 ms | 4.3 M, 0.60 ms | 3.45 M, 0.60 ms |
| Thread scroll, 100 turns | 55.3 M, 3.0 ms | 6.8 M, 0.71 ms | 3.45 M, 0.62 ms |
| Thread scroll, 400 turns | 212.7 M, 23.4 ms | 16.8 M, 1.57 ms | 3.45 M, 0.62 ms |
| Thread stream, 400 turns | 213.7 M, 23.2 ms | 16.6 M, 1.64 ms | 16.6 M, 1.08 ms |
| History scroll, 500 lines | 50.4 M, 4.5 ms | 3.9 M, 0.59 ms | 3.9 M, 0.56 ms |
| History scroll, 2,000 lines | 193.3 M, 22.4 ms | 3.9 M, 0.60 ms | 3.9 M, 0.57 ms |

- **Before.** The frame grows linearly with the session. At 400 turns or 2,000 history lines it is over the 16.7 ms budget of a 60 Hz frame, and three times over the 8.3 ms budget of a 120 Hz one.
- **List.** History frames no longer depend on how much history is loaded. Thread frames still grow, because every row's text is rebuilt per frame: 12.5 M of the 16.8 M at 400 turns.
- **Cached rows.** A scroll frame is flat at 3.45 M at every size. A streaming frame still rebuilds its rows, because the thread changes on every delta, so that row is unchanged.

The limiter after the change is GPUI's per-frame floor for this panel: text layout of the roughly 6 visible rows, plus paint and present. It is the same 3.4 to 3.9 M at every size.

## Files

- `run1.log` to `run3.log` hold before and list. `cache1.log` to `cache3.log` hold list and cached. All are raw Criterion and GPUI bench reports.
- `parse.py` produces `summary-list.txt` and `summary-cache.txt`: `python3 parse.py run*.log`.
