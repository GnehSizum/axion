# Input and Device Validation

The hello example includes `frontend/input-lab.html` for manual IME, selection, DPI, mouse and trackpad validation. Start the native example, then follow **Open native input and device validation**:

```sh
cargo run -p hello-axion --features servo-runtime
```

Do not enable `AXION_GUI_SMOKE` for this session: the normal smoke page automatically exits after its checks. Browser-only testing can check the page UI, but cannot establish Axion's native input mapping.

## What the page records

The page uses external CSS and JavaScript, does not install the selection compatibility patch, and never synthesizes keyboard, input or composition events. It shows `input`/`textarea` state, composition event order, `isComposing`, `isTrusted`, DPR, viewport size, CSS client coordinates, wheel units, scroll offsets and focus. Only the last 80 events are retained; counts cover the session since the last clear.

**Generate and select JSON** produces text for manual Cmd+C / Ctrl+C copying. It does not claim that the system clipboard was written. The report omits field values, composition data, character keys, URLs, tokens and clipboard contents. Lengths and selection offsets use UTF-16 code units, not grapheme counts. It is a local input-lab diagnostic, not an Axion GUI smoke report, and its verification state remains `manual-not-recorded`. `isTrusted` is useful context, not independent proof of a system input-method test.

Native Copy/Cut/Paste in this page uses Servo's DOM text-editing clipboard delegate. With the engine `clipboard` feature enabled, this accesses the operating system clipboard; an access failure can use Servo's own in-process fallback. Axion's `[native.clipboard] backend = "memory"` applies only to bridge `clipboard.*` commands and does not select this editing backend. Test native editing separately from bridge clipboard roundtrips. This Cargo feature does not itself enable the asynchronous `navigator.clipboard` API.

## Manual checks

Use only test text such as `Axion 中文🙂`. Record the actual system, input method and devices separately.

1. In both fields, use a real Chinese input method to compose, change candidates and commit. Check composition order and the visible final value; no duplicate commit or extra newline should appear.
2. Cancel preedit with Escape, switch to English, insert emoji, paste test text, select a range and replace it. Compare ordinary Enter in the textarea with Enter used to commit a candidate.
3. Move between the two fields, another app and back while composing. The new field must not be unexpectedly blurred or receive old preedit text. Confirm normal editing resumes after cancellation.
4. Move the window between displays with different scale factors. Record DPR/viewport before and after; verify visible pointer hit position, selection and candidate-box position. Coordinates shown on the page are CSS logical pixels.
5. Scroll the probe with a physical mouse and a trackpad, including horizontal movement. Check both wheel deltas/mode and actual scroll offsets. Synthetic events or mapping tests do not replace device results.
6. Generate the diagnostic JSON after each scenario. Keep the report together with the manual outcome; the page does not derive pass/fail from event counts.

## Device matrix

A row is complete only when the actual device scenario was performed. Leave missing hardware or platforms **unverified**. Native Computer Use results below establish the tested operating-system input path; they do not complete the physical-device rows.

| Platform / architecture | OS and Axion build | Input method / keyboard | Displays and scale | Mouse / trackpad | Scenario | Result | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| macOS arm64 | macOS 27.0.1 (26A434), local v0.6.1 M1 debug build | Active Chinese system source; source name not recorded; native automation | DPR 2; CSS viewport 1470 × 890 | Native automation | Commit / source switching / selection / system clipboard | Partial: listed native scenarios passed; physical-device rows remain unverified | Native input-lab JSON + visible test text |
| macOS arm64 | Record actual versions | Physical keyboard and named Chinese IME | One display | Record device | Text / commit / cancel / focus | Unverified | JSON + observed result |
| macOS arm64 | Record actual versions | Same as above | Two displays with different scale | Physical mouse and trackpad | DPI / hit position / scrolling | Unverified | JSON + observed result |
| Linux | Record distro, session and architecture | Record IME | Record displays and scale | Record device | Repeat supported scenarios | Unverified | Actual native run required |
| Windows | Record OS and architecture | Record IME | Record displays and scale | Record device | Repeat supported scenarios | Unverified | Actual native run required |

Ordinary text controls are the scope. `contenteditable`, rich text, complex bidirectional editing and uniform scrolling feel across platforms remain outside this acceptance page. See [security.md](security.md) for the bridge boundary and [architecture.md](architecture.md) for the runtime model.

## Native session recorded on 2026-10-05

The local debug hello bundle was built with `servo-runtime,lifecycle-probe` and the authorized Servo `clipboard` feature. The lifecycle probe was not activated in this interactive session. Computer Use sent native keys, clicks and scroll commands to the Axion window; no DOM input/composition events or selection patch were used. Accessibility/window capture permissions worked. The active source converted Latin preedit to Chinese, but its displayed name was not recorded. Input sources may be remembered separately for each application, so an English TextEdit session was not counted as a same-IME cancellation comparison.

Observed successful scenarios:

- System paste inserted `Axion 中文🙂` with UTF-16 length 10. Cmd+A/Cmd+C in input followed by Cmd+V in textarea retained the same text. Axion → TextEdit and TextEdit → Axion transfers also retained Chinese and emoji; the reverse test text had UTF-16 length 9.
- `ni` followed by Space, then `hao` followed by Space produced `你好`. All-selection replacement produced only the newly committed character. A separate, stabilized `ni` + digit 2 selected `尼`; candidate-box placement was not captured.
- Switching to English with Ctrl+Space and typing `abc`, then switching back and committing `ni`, produced `你好abc你` without clicking the field again. The DOM focused target remained input.
- The first exported session recorded three composition starts, ten updates and three ends. Each cycle began before its updates and ended once. The final field length was 6 and both fields were outside composition. This validates those cycles, not every DOM input-event ordering property.
- In a stabilized textarea scenario, Enter accepted Latin `nin` from preedit with length 3 and no newline; the following ordinary Enter changed length to 4. Pasted `abc` followed by one ordinary Enter also changed length from 3 to 4.

A rapid multi-key batch produced delayed intermediate snapshots and the copied value `ni\n\n好 ` (length 6). The stabilized single-Enter scenarios did not reproduce duplicate newline insertion. Keep this observation pending physical typing / timing comparison; do not implement a blanket rule that discards the next Enter or Space after a commit.

The second exported session exposed an actual cancellation-event gap: Escape produced an empty composition update and subsequent non-composing key events, while the page still considered the field composing. It recorded four starts and three ends; normal blur cleared the page flag. The adapter now waits until the native batch ends before sending an empty composition end when no commit follows. The 2026-10-06 rerun below verifies this repair. Keeping raw Latin after unmarking is a separate platform semantics question: [AppKit unmarkText](https://developer.apple.com/documentation/appkit/nstextinputclient/unmarktext%28%29?changes=_7_4_1&language=objc) describes accepting marked text as ordinary text, so raw-text retention alone is not proof of a cancellation defect.

Local evidence: `target/axion/reports/native-input-first.json`, `native-input-second.json`, and the corresponding native screenshots. The JSON files are page-generated diagnostics, retain their original `manual-not-recorded` status, and contain lengths/offsets rather than test text. Human observations in this document supply the scenario outcome. Physical keyboard, mouse, trackpad, horizontal scrolling, mixed-DPI transitions, candidate position and composition-time field/app switching remain unverified.

The captured 1920-CSS-pixel window showed a white region beginning near physical x=2430. A resize to 1470 CSS pixels restored complete capture; Hello → input-lab navigation at that smaller size also rendered completely, while restoring 1920 reproduced the white region. A read-only display/window query established one logical 1470 × 956 display at DPR 2 and the large window at bounds (255, 34, 1920, 922). Its screen-visible width, (1470 − 255) × 2 = 2430 physical pixels, exactly matches the white-region boundary. The white region lies outside the screen and is recorded as an off-screen capture/compositing limitation; it does not establish a Servo clipping defect within the display. The resize/render code was left unchanged.

## Cancellation rerun recorded on 2026-10-06

In the repaired native bundle, `zh` preedit first showed a composing field. One Escape ended that state without blur, collapsed the selection, and retained input focus. The exported diagnostic records empty update at sequence 7 followed by exactly one empty composition end at sequence 9. Raw `zh` remains ordinary text; this repair completes events and does not implement deletion of the platform's accepted text.

Subsequent all-selection replacement and two Space commits produced `你好`. Switching to English and back without clicking produced `你好abc你`. Switching input source during a further `ni` preedit produced an ordinary Latin commit: empty update at sequence 57, composition end of UTF-16 length 2 at sequence 58, then non-composing input. The next English character inserted normally while focus stayed in the same field. This macOS session used a commit before Disabled; the legal direct nonempty-Preedit → Disabled mapping is separately covered by unit tests.

The exported rerun records five starts, sixteen updates and five ends, with exactly one end per observed cycle; both fields end outside composition. Evidence is `target/axion/reports/native-input-final.json` and `native-input-final-escape.png`. The diagnostic's original `manual-not-recorded` value is preserved; the observations here describe the verified native scenarios. The physical-device and candidate-position limits above still apply.
