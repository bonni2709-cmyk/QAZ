# Building the Android APK

The APK is minted by the **`apk`** job in `.github/workflows/build.yml`. It is
intentionally *not* built on every push: it is manual-only
(`workflow_dispatch`), because it downloads the Android SDK/NDK toolchain and
produces a ~54 MB artifact.

## Run it

1. Actions tab → **build** → **Run workflow** → pick this branch.
2. Wait for the `apk (manual, cargo-quad-apk)` job (~7 minutes).
3. Download the **`railgun-pursuit-3d-apk`** artifact (`.apk`, ~54 MB) from the
   run's Summary page. Artifacts expire after 30 days.

With the GitHub CLI instead: `gh run download <run-id> -n railgun-pursuit-3d-apk`

## What the job does

1. Generates the asset pack (`tools/gen_assets.py`) and verifies the manifest
   (≥ 10 files, ≥ 40 MB) — the APK ships the real generated pack.
2. Installs stable Rust with the `aarch64-linux-android` and
   `x86_64-linux-android` targets (armv7 was dropped: modern NDKs no longer
   ship `libunwind.a` for 32-bit ARM).
3. Locates the oldest Android NDK that ships `llvm-ar` and creates the
   pre-r23 legacy symlinks (`<triple>-ar/-ld/readelf/...` → `llvm-ar`/`ld`/...)
   that `cargo-quad-apk` still probes for.
4. Passes per-target `-L <ndk>/lib*/clang/<ver>/lib/linux/<arch>` so the bare
   `<triple>-ld` link can find `libunwind.a`.
5. Installs `platforms;android-36` (miniquad's `MainActivity.java` needs
   API 30+ symbols such as `Window.setDecorFitsSystemWindows`).
6. Installs `cargo-quad-apk` **from the pinned master revision**
   `d411c8fe` — the crates.io release (0.1.4) is the pre-2025 codebase that
   demands a Java 8 `rt.jar`, which no modern JDK ships. Master instead uses
   aapt2 + d8 + `-bootclasspath android.jar` and any modern JDK.
7. Runs `cargo quad-apk build --release` and uploads
   `target/android-artifacts/release/apk/*.apk` as the `railgun-pursuit-3d-apk`
   artifact (signed with the default debug keystore).

## Install on a device

Enable "Install unknown apps" for your browser/file manager, then open the
APK. Target: Infinix Note 30 (Helio G99, aarch64). `x86_64` is included for
emulators/ChromeOS. The APK is debuggable and signed with the debug key —
fine for sideloading; use a real keystore before any store release.

## Bumping

- `version_code` / `version_name` live in `Cargo.toml`
  `[package.metadata.android]` — bump them per release.
- `android_version`/`target_sdk_version` are 36; keep them in step with the
  `platforms;android-XX` install in the workflow.
- `build_targets`: add a Rust triple + linker row + `-L` arch mapping to
  expand ABIs. Remember each added ABI needs a `libunwind.a` in the NDK
  (32-bit ARM does not provide one anymore).
