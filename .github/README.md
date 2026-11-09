<div align="center">
  <img src="../assets/icon/icon.png" alt="ytm-rs icon" width="96">
  <h1>ytm-rs for Android</h1>
  <p><strong>An experimental Android port of <a href="https://github.com/grapedjuice/ytm-rs">ytm-rs</a>, the fast native YouTube Music client.</strong></p>
  <p>The same Rust app and egui interface, adapted to a phone held upright.</p>

  <p>
    <a href="https://github.com/gabialonso/ytm-rs-android/releases"><img src="https://img.shields.io/github/v/release/gabialonso/ytm-rs-android?include_prereleases&style=flat-square&color=ff4e45&label=release" alt="Latest release"></a>
    <img src="https://img.shields.io/badge/Android-13%2B%20%C2%B7%20arm64-333?style=flat-square" alt="Android 13+, arm64">
    <a href="../LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-333?style=flat-square" alt="License: GPL-3.0"></a>
  </p>
</div>

> [!IMPORTANT]
> This is an unofficial fork, not affiliated with ytm-rs or with Google. It is an
> experimental pilot, written largely with an AI coding assistant, so expect rough
> edges. Please report problems here, not to ytm-rs. For the desktop app (Windows,
> macOS, Linux), go to **[grapedjuice/ytm-rs](https://github.com/grapedjuice/ytm-rs)**.

## Install

Download the `.apk` from the [latest release](https://github.com/gabialonso/ytm-rs-android/releases)
and open it on the phone (allow installing from your browser or file manager when
Android asks). It needs **Android 13 or newer** on a 64-bit ARM phone (arm64-v8a, which
is nearly every phone from the last several years).

Sign in with your Google account from the app; the session stays on the phone, in the
app's private storage.

## What works

- Home, Explore, search, library, playlists, artists and albums.
- Playback, also with the screen off, with controls on the lock screen, in the
  notification and on headphones.
- The queue, Now playing, and synced karaoke lyrics, with the desktop app's font and
  lyrics settings.
- A phone layout: tabs and a mini player at the bottom, Now playing full screen, swipe
  gestures, the system Back button.

Left out on purpose: themes other than the default one, music videos, uploads and
downloads, Spotify import, Discord, Last.fm, jams, auto-update (check the releases page)
and choosing the output device.

## Versions

The Android app has its own version, `android/version.properties`, released as tags
`android-vX.Y.Z`. Each release names the ytm-rs version it's built on, which is also
shown in Settings › About.

## Building from source

You need Rust with the `aarch64-linux-android` target, [cargo-ndk](https://github.com/bbqsrc/cargo-ndk),
JDK 17 and the Android SDK with NDK 29 (`ANDROID_HOME` set). Then:

```sh
ABIS=arm64-v8a android/build.sh release    # optimized, a few minutes (fat LTO)
adb install -r android/app/build/outputs/apk/release/app-release.apk
```

`android/build.sh debug --install` builds faster and installs on the connected device; it
also builds x86_64, for the emulator. Release builds are signed with the key named in
`android/keystore.properties` if there is one, and with the debug key otherwise.

## How the fork is organized

- `android`: the port, and the default branch. `main` mirrors upstream's `main`, so
  new versions of ytm-rs come in with a merge.
- The Android code is in `src/android/` (JNI, the playback service bridge, the app's
  lifecycle), `src/ui/mobile.rs` (the phone layout) and `android/` (the Gradle project
  and its Java side). Shared files only get small hooks, to keep those merges easy.

## Credits and license

ytm-rs is by [grapedjuice](https://github.com/grapedjuice) and contributors; everything
not Android-specific here is theirs. Like ytm-rs, this fork is under the
[GPL-3.0](../LICENSE).
