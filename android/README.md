# ytm-rs for Android (experimental)

The same egui app, built as `libytm_rs.so` and run by a `NativeActivity`. The Java side
(`app/src/main/java/...`) is small and uses platform APIs only (minSdk 33, no AndroidX):

| Class | Does |
|---|---|
| `MainActivity` | Runs the Rust app; Back sends it to the background (the player keeps going) |
| `Bridge` | JNI calls both ways (Rust side: `src/android/`) |
| `LoginActivity` | Google sign-in in a WebView; hands the youtube.com cookies to Rust |
| `Player` | MediaSession (lock screen, headset, Bluetooth), audio focus, unplugged headphones, wake/Wi-Fi locks |
| `PlaybackService` | Foreground service while music plays, and the media notification |

On the Rust side `src/lib.rs` (Android only; empty on desktop) declares the same modules
as `main.rs`, with Android versions of the desktop-only ones in `src/android/`. While the
app has no window (screen off), a thread runs `App::tick` so the queue keeps moving.

## Build

Needs the NDK, `cargo-ndk`, JDK 17 and `LIBCLANG_PATH` pointing at the NDK's
`toolchains/llvm/prebuilt/linux-x86_64/lib` (rquickjs generates its bindings for Android).
On the dev machine all of that is set by `source ~/.local/opt/android-env.sh`.

```sh
android/build.sh               # debug APK (arm64 phone + x86_64 emulator)
android/build.sh release       # optimized; slow to build (fat LTO)
android/build.sh debug --install   # and adb install it
```

The APK lands in `android/app/build/outputs/apk/<profile>/`. With more than one device
connected, set `ANDROID_SERIAL` for `--install`.

## Known limits (pilot)

- Swiping the app away from Recents ends it, music included: winit can make only one
  event loop per process, so the Rust side can't start over without a new process.
- The layout is still the desktop one; the phone layout is the next phase.
- No themes beyond the default, no Spotify/jam/Last.fm/Discord/translation/SponsorBlock,
  no video, uploads or downloads (see HANDOFF.md).
