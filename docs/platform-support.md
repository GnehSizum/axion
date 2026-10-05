# Platform Support and Validation Scope

Axion **v0.6.2 developer preview** uses vendored Servo **v0.6.0**. This page records observed validation as of **2026-10-06**. A configured backend or CI job is not a platform support guarantee.

## Validation matrix

**Passed** means the stated scenario ran successfully. **Failed** means an actual run failed. **Unverified** means the scenario has no successful evidence in that environment. The recorded macOS M1 GUI, input, lifecycle and performance results used local v0.6.1 builds; the linked Linux CI run uses the earlier `156419a3` baseline and does not contain those changes.

| Area | macOS arm64 | Linux x86_64 (Ubuntu 24.04 CI) | Windows |
| --- | --- | --- | --- |
| Default Rust checks | Passed locally for v0.6.2 | Passed on the linked baseline run | Unverified |
| Servo feature build | Passed locally for v0.6.2 with Rust 1.97.1 | Failed in native dependency compilation; local compiler-selection fix awaits CI | Unverified |
| Native example GUI | v0.6.2 hello passed 14 checks; earlier four-example run passed 55 checks before the later IME/clipboard fixes | Job failed before successful GUI acceptance | Unverified |
| External source SDK apps | Two templates passed 36 GUI checks; later native CLI frontend watch/reload and same-SDK path rebinding passed | Unverified | Unverified |
| Ordinary text / Chinese IME | Native commit, input-source switching and Escape completion passed; 26 native tests passed; hardware acceptance remains partial | Unverified | Unverified |
| Mouse / trackpad / mixed-DPI displays | Mapping tests exist; physical devices and display transitions unverified | Unverified | Unverified |
| Backend release / surviving host | Debug and release probes passed in 24 isolated scenarios with 72 post-return heartbeats | Unverified | Unverified |
| System clipboard / dialogs / URL opener | Native DOM text copy/paste passed within Axion and both ways with TextEdit; system dialogs and URL opening remain unverified | Unverified | Unverified |
| Local release / unpacked launch | Unsigned .app archive passed 18 checks after unpacking, hiding the source app and changing cwd | Baseline release-preview failed; repaired workflow unverified | Unverified |
| Clean target machine / signed distribution | Unverified | Unverified | Unverified |

The measured macOS environment was **macOS 27.0.1 (26A434), Apple M4, arm64, 24 GiB RAM**, using **Rust 1.97.1**. This establishes one tested environment, not a minimum macOS version or compatibility with every arm64 Mac. Axion declares Rust **1.88.0**; that toolchain has not been tested in this validation, with or without Servo. Use the repository's pinned toolchain for the currently reproduced build.

[CI #34](https://github.com/GnehSizum/axion/actions/runs/37324830292) passed the default Rust job but failed native-check, GUI and release-preview jobs. Both native-check and release-preview logs show LLVM 18 headers mixed with libclang 19 during mozangle/bindgen compilation; the release stopped in the bundle build phase before unpacked deployment. The GUI job confirms a Cargo build failure, but its detailed compiler cause has not been established. The local workflow now explicitly selects Clang 19 for header discovery as well as compilation and libclang loading; this repair still needs a successful remote run. See [CI validation](ci.md) for compiler diagnostics and report collection.

## What the local results establish

Before committing v0.6.2, local validation reran 325 workspace tests, 26 Servo-enabled window tests, the bridge JavaScript and lifecycle harness tests, native compilation for the CLI and four examples, all four example self-tests, the aggregate check and hello GUI smoke (14 checks). Formatting, diff checks and clippy completed successfully; clippy still reports style warnings. The earlier external release, input-method, device, lifecycle and performance observations retain their original v0.6.1 scope.

External `vanilla` and `native-api-demo` applications used an explicitly selected source SDK and retained the generated toolchain/platform configuration. Their native builds reused cached dependencies and **seeded the application Cargo.lock from the SDK lock**. Both templates passed 18 GUI checks each. These template GUI checks preceded the later IME/clipboard fixes. The generator does not automatically seed a lock; a fresh unconstrained registry resolution was not validated for native compilation. See [getting started](getting-started.md) and [CLI](cli.md) for SDK selection and rebinding.

A later external `vanilla` session ran `dev --launch --watch --reload` with the native CLI host. Development app.ready/app.ping, one frontend reload and managed frontend-server shutdown passed. The three Axion dependencies were also rebound to another Unicode/space symlink path to the same SDK, followed by successful doctor/check; another SDK version was not tested. CLI dev uses its generic host, so this does not validate the external Rust main, custom DemoPlugin, Rust rebuild, restart loop or a cold build.

The vanilla release archive was unpacked and launched from another working directory while the source application's directory was hidden. Its 18 checks passed without restoring the source app first. The SDK remained on disk and the test used the development machine, so this does not establish clean-machine installation or signed distribution. See [packaging](packaging.md) and [release checks](release-checks.md).

The recorded lifecycle probes preceded the later IME/clipboard fixes. They cover normal confirmed close, an actual missing-resource request followed by close, synthetic partial-window creation failure and synthetic rendering failure. Each scenario ran three times in separate processes in both debug and release. The checks establish released AppState ownership, empty window/binding registries, backend return and a responsive surviving host. Remaining process-wide pools and engine threads were observed; complete Servo thread shutdown is not established.

IME state tests verify event mapping and control identity. Initial native Chinese commit and selection replacement were observed. After enabling the Servo clipboard feature, test text including Chinese and emoji pasted successfully, input-to-textarea copy/paste passed, and Axion-to-TextEdit plus TextEdit-to-Axion text transfers passed. This DOM editing path is separate from Axion bridge `clipboard.*` commands and their memory backend. Consecutive ni/hao commits and switching to English and back with Ctrl+Space passed without clicking the field again; focus stayed on the input and each observed composition cycle had one start and one end. Escape produced an empty preedit without compositionend in the earlier diagnostic. The repaired native rerun ended composition after Escape without blurring the field and preserved input focus. Its diagnostic records an empty update at sequence 7 followed by one empty end at sequence 9; five starts, sixteen updates and five ends cover Escape, three Chinese commits and a Latin commit during input-source switching. Both fields finished outside composition. All 26 native window tests passed, including the separate direct nonempty-Preedit-to-Disabled boundary. The tested macOS source switch instead sent an empty preedit and a commit before Disabled, so that direct boundary was not reproduced in this UI session. Raw Latin text retention remains a separate platform semantics observation; the repair completes events rather than deleting accepted text. GUI text snapshots and Unicode insertion alone do not prove native Chinese candidate composition. Follow [input and device validation](input-validation.md) to record actual input methods, candidate commit/cancel, selection, focus, scrolling and display scale. Ordinary input/textarea controls are the present scope; rich text and complex bidirectional editing remain unverified.

A large-window screenshot showed a white right-hand region exactly where the window extended beyond the physical display. A smaller, fully visible window rendered completely. The evidence identifies an off-screen capture/compositing limitation; candidate placement, physical pointing devices and mixed-DPI display transitions remain unverified.

## Initial performance sample

These measurements were collected before the later IME/clipboard fixes and describe that recorded binary, not the latest corrected build. They use release `hello-axion` with the test-only `lifecycle-probe` feature, strip disabled, JIT disabled and dummy media. Five separate processes were sampled for each window count, keeping the frontend open for three seconds before requesting close. Values are **median [minimum, maximum]**.

| Metric | One window | Two windows |
| --- | --- | --- |
| Launch to all windows registered (ms) | 118.720 [106.837, 147.743] | 123.592 [116.221, 170.335] |
| Launch to all frontend.ready records (ms) | 165.122 [147.291, 187.676] | 168.442 [165.745, 220.262] |
| First close request to backend return (ms) | 48.274 [40.251, 54.107] | 55.911 [53.277, 70.160] |
| Maximum sampled RSS (MiB) | 153.703 [153.141, 154.188] | 172.828 [172.312, 174.156] |
| RSS sampled after backend return (MiB) | 147.016 [146.688, 147.812] | 159.594 [159.406, 159.906] |

Timing starts in the harness just before temporary setup and process creation and includes pipe/scheduling overhead. Window registration and frontend IPC readiness are not first-frame presentation timestamps. RSS was sampled with ps approximately every 250 ms, so transient peaks may be missed. Cache state was uncontrolled; these are neither cold-start measurements nor latency percentiles. Instrumentation and the small sample count limit comparisons with production applications.

The instrumented release binary was 145,109,728 bytes. Its cached incremental build took about 158 seconds. The external vanilla app archive was 145,062,400 bytes and its cached release/bundle/archive step took 26.582 seconds. These are individual build samples, not download size or build-time promises. Full first builds and clean-machine dynamic dependencies still need validation.

Local raw evidence is retained under `target/axion/reports/`: `v062-hello-gui-smoke.json`, `v062-check.json`, `v062-*-self-test.json`, `m1-*-gui-smoke*.json`, `m1-post-clipboard-hello-gui-smoke.json`, `m1-final-ime-hello-gui-smoke.json`, `native-input-final.json`, `native-input-final-escape.png`, `lifecycle-probe-{debug,release}.json`, `lifecycle-release-{single,double}.json` and `m1-baseline-summary.json`. These generated files are ignored by Git. Reproduce the native and release gates from [CI validation](ci.md); record device outcomes separately from automated report counts.
