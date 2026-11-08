//! The Android build: a cdylib whose `android_main` NativeActivity starts.
//!
//! Desktop builds keep using `main.rs` untouched (this file is empty there), so merging
//! upstream stays simple. The module list mirrors `main.rs`; modules that only make
//! sense on a desktop are swapped for the Android versions in `src/android/`.
#![cfg(target_os = "android")]
// Desktop-only helpers in the shared modules go unused here.
#![allow(dead_code, unused_imports)]

mod android;

mod art;
mod audio;
mod backend;
mod bidi;
mod images;
mod innertube;
mod jsc;
mod lyrics;
mod model;
mod opus;
mod preset;
mod stream;
mod theme;
mod ui;
mod vault;
mod webmusic;
mod ytm;
mod ytmapi;

// Kept as is: they build on Android and their desktop-only parts are already behind
// `cfg(windows)` or only run when a setting asks for them.
mod appicon;
mod backdrop;
mod deeplink;
mod downloads;
mod fullscreen;
mod jam;
mod lastfm;
mod proxy;
mod renderer;
mod shader;
mod sponsorblock;
mod transfer;
mod translate;
mod update;
mod ytdlp;

// Replaced on Android.
#[path = "android/discord.rs"]
mod discord;
#[path = "android/login.rs"]
mod login;
#[path = "android/media.rs"]
mod media;
#[path = "android/tray.rs"]
mod tray;
#[path = "android/turnstile.rs"]
mod turnstile;
#[path = "android/video.rs"]
mod video;

/// Window size on first launch, and the smallest it may be (points). A phone has a
/// fixed window; these only feed the desktop code paths that still read them.
pub const DEFAULT_SIZE: [f32; 2] = [400.0, 800.0];
pub const MIN_SIZE: [f32; 2] = [320.0, 480.0];

pub use android::android_main;
