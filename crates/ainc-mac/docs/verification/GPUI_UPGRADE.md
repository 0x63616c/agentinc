# Current GPUI upgrade acceptance

Verified on 23 September 2026 on Apple M2 Pro, macOS 27.0 (26A428), with `rustc 1.98.1 (48a229cea 2026-09-01)`.

## Dependency provenance

Agentinc OS now uses Zed git commit **`4c902c9db22a82f5f3a14c02442e7f60ec40d9c8`**, the inspected upstream main revision, instead of the crates.io GPUI 0.2.2 release. Both direct GPUI dependencies have an exact `rev` and version requirement; `Cargo.lock` records their shared source. Zed still calls its core crate `gpui 0.2.2` internally: that metadata does **not** mean Cargo uses the old published release.

| Resolved package | Version | Source revision |
| --- | --- | --- |
| gpui | 0.2.2 | `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` |
| gpui_platform | 0.1.0 | same |
| gpui_macos | 0.1.0 | same |
| gpui_apple | 0.1.0 | same |
| zed-font-kit | 0.14.1-zed | `94b0f28166665e8fd2f53ff6d268a14955c82269` |

The Cargo sparse registry still listed `gpui-pre 0.3.6` as its newest snapshot at lookup. The pinned Zed revision contains merged [renderer texture safety change](https://github.com/zed-industries/zed/pull/64623), merge commit `96e95edac45b0e0696179b8139aa521cf48db392` (GitHub compare reports 24 commits ahead). No fork or `[patch]` is used. Core and macOS platform `font-kit` remain enabled; `runtime_shaders` is now a platform feature. The toolchain pin is 1.98.1.

The source migration updates startup to `gpui_platform::application`, focus calls to take App context, text painting arguments, menu fields and the window-closed callback. The Unicode test comes from the trial. The trial polling capture hook and incomplete shared-button accessibility probe are removed. Product layout and interaction code retain their behavior.

## Reproduction result and correction to the trial report

The trial capsule was extracted from the supplied report and its archive SHA256 verified as `5bc936c767bc9b4597c8f6d8a9fde3c9f6b2f224f3f2e77dd6342d4fa4bddaa1`. Its `final.patch` was applied to base `cd05f1094bcd9e4d73a574967c3819c8c171e022` before changing dependencies.

**The reported missing-shell defect was not reproduced.** Before testing the newer revision, the exact trial dependency/lockfile produced complete native and self-captured Tasks → Today frames, including after native zoom resize. Inspection of the capsule's original allegedly failing images also contradicts the report: the header, all sidebar labels, profile, borders and Evee are present. The task-dialog image has the intended dark backdrop and a complete dialog.

- [Original trial return to Today, reduced to logical size](gpui-upgrade/trial-capture-today-2.png)
- [Original trial resized return to Today](gpui-upgrade/trial-capture-today-5.png)
- [Original PNG hashes and independent region pixel counts](gpui-upgrade/trial-pixel-audit.json)

For example, the original `capture-today-5.png` contains 1,621 bright header pixels, 1,888 profile pixels and 1,120 Evee-header pixels; each of its eight sidebar labels has 616–928 bright pixels. Counts use RGB maximum >90 (border >20), on the unmodified original Retina PNG. Its SHA256 is `f53801725b4f49606a8ce501d8f51a0b36c6592612bcfe4570af1dd2b077284c`.

Some inline computer-tool previews appeared to omit unchanged regions during this run. Saving the returned full PNG and inspecting it at logical resolution showed those regions present. This is evidence of a preview/interpretation problem, not proof of an app renderer failure. The new regression therefore checks saved scene pixels and includes deliberately blanked-region negative controls.

The same final 38-frame rendered test was also run in an ignored isolated subproject against **gpui-pre 0.3.6**, upstream `bcf6582ce3500df93a8a39366640173e6786cea6`; all frames passed. Thus this upgrade does **not** claim that the merged renderer change fixed the reported symptom. There is no demonstrated app or upstream renderer defect requiring a workaround.

## Automated checks

```sh
cargo fmt --all --check
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --features rendered-tests --test rendered_shell
scripts/bundle.sh
codesign --verify --deep --strict --verbose=2 'dist/Agentinc OS.app'
```

The worker used worktree-local `CARGO_HOME` and `RUSTUP_HOME` with the installed 1.98.1 compiler, preserving shared caches/toolchains. These environment overrides are not needed for normal development.

| Check | Observed result |
| --- | --- |
| Format | Passed |
| Nonvisual tests | 18 passed, 1 existing live-Codex test ignored |
| Clippy, all targets and all features | Passed with warnings denied |
| Real Metal regression, pinned Zed | 38 captured frames passed |
| Same Metal regression, trial 0.3.6 | 38 captured frames passed |
| Normal bundle and ad hoc signature | Passed |

The nonvisual tests exercise Unicode typing/backspace/undo/replacement through GPUI input dispatch; route shortcuts and back/forward; Search filtering, selection, empty results, focus wrapping and Escape restoration; task input/Return submission, trimming, completion/reopening/deletion and empty-title rejection. Test fixtures use private temporary session/database files and a synthetic profile. Tests skip the initial Codex status request; normal app startup is unchanged.

`tests/rendered_shell.rs` is a custom main-thread harness because AppKit cannot run on a normal Rust test worker. It uses the real product modules, `VisualTestAppContext`, real assets, macOS text and Metal `Window::render_to_image`. It navigates all nine destinations over three rounds, opens/dismisses Search and Add task, types Unicode, and hides/restores Evee. Every frame checks header, workspace, each sidebar row, profile and panel border; visible Evee adds header, composer and border checks. Removing each region from the first frame independently fails its assertion. This detects absent regions; it is not a golden-pixel typography comparison.

The harness verifies two real offscreen window sizes: 1360×828 and 1160×728 logical pixels (2720×1656 and 2320×1456 captured pixels). AppKit resize is asynchronous without the native event loop, so the smaller size uses a second offscreen window. **Actual same-window resize/navigation was tested natively below.** No forced `refresh()` workaround is used by the regression. Artifacts default to `target/rendered-shell/`; `AGENTINC_RENDER_OUTPUT` can change the output directory.

The rendered target requires `--features rendered-tests`; its macOS runner is cfg-gated and prints a skip on other platforms. Linux CI continues to run the nonvisual tests. Local checks above do not claim Linux execution or hosted CI success. Cargo also emits the existing `block 0.1.6` future-compatibility notice.

## Native acceptance and visual parity

The normal bundled executable was copied to an isolated QA bundle with a distinct identifier. All four README isolation variables were set and also placed in that QA bundle's `LSEnvironment`, so tool reattachment could not fall back to normal app data. Session/database/Codex state remained under `.local/gpui-upgrade/native-current/`; no account sign-in or live reply was attempted.

- Navigated all eight shortcut routes, then Settings via Search. Repeated all eight after native zoom from 1360×828 to **3840×1504** logical pixels, then restored the window and checked Tasks and Today again. Saved complete native screenshots at both sizes.
- Opened Search, filtered to Settings, selected with Return, and separately dismissed Search with Escape.
- Opened Add task with the pointer, typed `GPUI shipping check`, traversed the dialog with Tab twice and activated Create with Return. The saved isolated database contains that task.
- Opened Notifications and dismissed it with Escape.
- Hid Evee, navigated Tasks → Today, restored Evee and inspected the recovered panel and composer.
- Compared the current native shell at logical resolution with the verified 0.2.2 baseline from the trial capsule, the Control desktop reference and the approved refinements in `STATUS.md`. Text remains visible and legible; shell geometry, compact typography, icons, panel borders, spacing and tab contour remain consistent. This is visual parity review, not exact pixel equality across framework text rasterizers.

| Evidence | Capture |
| --- | --- |
| Same-size old/current comparison | [Comparison](gpui-upgrade/visual-comparison.png) |
| Native Today | [Today](gpui-upgrade/native-today.png) |
| Native Tasks | [Tasks](gpui-upgrade/native-tasks.png) |
| Native Add task | [Dialog](gpui-upgrade/native-dialog.png) |
| Native Evee hidden | [Hidden](gpui-upgrade/native-evee-hidden.png) |
| Native resized return to Today | [Resized](gpui-upgrade/native-resized-today.png) |
| Self-capture after small-window navigation | [Metal frame](gpui-upgrade/rendered-route-1-0.png) |
| Self-capture with Unicode task dialog | [Metal dialog](gpui-upgrade/rendered-add-typed.png) |
| Self-capture after Evee restoration | [Metal restored](gpui-upgrade/rendered-evee-restored.png) |

Checked bundle: `dist/Agentinc OS.app`, built by the normal bundling script, ad hoc signed. No release optimization, notarization, Windows rendering, OS IME acceptance, full accessibility semantics, authenticated Evee reply or performance benchmark is claimed. The test runner is test-only; no file-polling hook or driver ships in the app.
