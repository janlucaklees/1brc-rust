## WyattAu — threaded chunked reader + open-addressing aggregation

**Local benchmark** (i5-9400F, 6 cores/6 threads, NVMe, release + fat LTO):
100M rows (1.73 GB) in 13.7 s wall / 28 s CPU under external memory
pressure; full 1B-row timings measured 5-run median on this box (see
comment below) — hardware differs from the eval machine, numbers are
informative only.

### Implementation notes

- Chunked `File::read` (std only, no mmap/unsafe — external deps
  disallowed), partitioned by byte range aligned to newline boundaries;
  scoped threads, one open-addressing table per worker.
- Table: 16384 slots (>= 1.6x the 10k-station ceiling), FxHash-style
  multiply-rotate byte hashing, linear probing, keys inlined in the
  entry (<= 100 bytes per spec).
- Temperatures parsed as integer tenths (branch-light digit loop);
  min/max in tenths, sum in i64 tenths (max 1e13, no overflow).
- Mathematical rounding replicated integer-exact from the baseline
  (`sum*10/count`, +10 when `|avg % 10| >= 5`, negative-safe).
- Merge folds per-thread aggregates create-with-aggregate / fold —
  no per-measurement double counting.
- Output identical to the reference on all 8 sample files including
  measurements-complex-utf8 (giant concatenated names, multibyte
  sequences) and measurements-boundaries (-99.9/99.9 extremes).

### Development methodology

Correctness was driven by the sample suite plus a dedicated
aggregates-verification pass (per-entry sum/count asserted against the
parsed row stream under load). Two bugs the samples caught during
development are documented in-source: a `return`-in-nested-loop that
aborted merging after the first entry, and an upsert-then-fold
double-count that seeded `min` as if it were a measurement.
