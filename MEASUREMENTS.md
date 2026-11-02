# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | rustc 1.99.0, wasm-bindgen 0.2.127 | 2026-11-02 |
| Core tests | `./scripts/count-tests.sh` | 113 passed, 0 failed, 0 ignored | 2026-11-02, after round 3 |
| Full check | `./scripts/check.sh` | BUILDER CHECK PASSED in chromium, firefox, webkit, webkit-iphone, webkit-ipad | 2026-11-02, after round 3 |
| WebAssembly size | printed by `./scripts/build.sh` | 393 KB (`opt-level = "s"`) | 2026-11-02 |
| Breaks caught | `python3 scripts/break_tests.py` | 37 of 37 | 2026-11-02 |
| Tempo found in the placeholders | `cargo run --release -p chart_tool -- analyse` | 100.007, 118.007, 132.006, 150.008, 172.007 bpm for songs made at 100, 118, 132, 150, 172 | 2026-11-02 |
| Targets per chart (Easy to Master) | `data/charts/index.json` | placeholder-1: 27, 87, 141, 175, 202; -2: 38, 111, 138, 240, 282; -3: 43, 123, 155, 255, 276; -4: 47, 77, 160, 238, 267; -5: 51, 86, 207, 293, 325 | 2026-11-02 |
| Frame times on the densest chart (BO-11) | `.venv/bin/python tests/browser/probe_frames.py chromium firefox webkit` | placeholder-5 Master, 325 targets, 15 s with presses: Chromium median 16.7 ms, 99th 16.8, 0 over 20 ms (also with `--disable-gpu`); Firefox median 8.3 ms, 99th 16.7, 8 over 20 ms; WebKit median 17.0, 99th 19.0, 0 over 20 ms. Headless, on a MacBook Pro (M2 Max) | 2026-11-02 |

## Not measured

Devices, browsers and conditions no measurement here covers.

- A laptop with no graphics card. `--disable-gpu` on this machine is the nearest stand-in; a school Chromebook is a person's check.
- Audio latency on real hardware: headless browsers have no sound card, so `outputLatency` there is not a laptop's. The calibration screen is the fix on each machine.
- Real touch timing on a phone or tablet (kits/mobile, "What emulation cannot show").
