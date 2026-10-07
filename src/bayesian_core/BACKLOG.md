# Bayesian core backlog

## Performance

- Replace `ExecutionState`'s `HashMap<InstanceKey, f64>` with dense, precompiled
  value slots. Parameter references should resolve directly to slot indices so
  inference avoids allocating plate-index vectors and hashing keys on every
  model execution. Benchmark representative plate sizes before and after, and
  consider skipping nested `ModelResult` materialization during warmup/scoring.
