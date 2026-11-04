# Check times

One row per run of ./scripts/quick-check.sh, newest last. A quick check skips relative to the newest passing `check` row.

| When | Commit | Check | Steps (s) | Total (s) | Result |
| --- | --- | --- | --- | --- | --- |
| 2026-11-02 13:54 | 8cb3325+ | quick | fmt=0.1 notebook=0.1 journal=0.1 | 0.2 | pass |
| 2026-11-04 17:20 | 8de0fd6+ | quick | fmt=0.2 core=3.1 | 3.3 | FAIL at core |
