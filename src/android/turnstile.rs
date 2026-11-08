//! Android: no Better Lyrics verification (it needs a webview in a child process).
//! The lyrics fallback moves on to the other sources.

use std::path::PathBuf;

pub fn obtain(_profile_dir: PathBuf) -> anyhow::Result<String> {
    anyhow::bail!("lyrics verification isn't available on Android")
}
