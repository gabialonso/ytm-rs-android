//! Cloudflare Turnstile for the Better Lyrics API on Android, done as the desktop build
//! does it (src/turnstile.rs): the API's own challenge page runs in a WebView (the Java
//! `Turnstile`) and its widget reports the token. It normally passes without being seen;
//! if Cloudflare wants an interaction while the app is on screen, the page shows in a
//! dialog. Nothing here solves or forges the challenge.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crossbeam_channel::Sender;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jboolean;

use crate::android::{call, jstr, rust_string};

const CHALLENGE: &str = "https://lyrics.api.dacubeking.com/challenge";
/// A little past the Java side's own two minutes.
const GIVE_UP_AFTER: Duration = Duration::from_secs(130);

static PENDING: Mutex<Option<Sender<Result<String, String>>>> = Mutex::new(None);

/// Run the challenge; returns the Turnstile token. Blocks (callers run it on a blocking
/// thread).
pub fn obtain(_profile_dir: PathBuf) -> anyhow::Result<String> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    *PENDING.lock().unwrap() = Some(tx);
    call("startTurnstile", "(Ljava/lang/String;)V", |env| Ok(vec![jstr(env, CHALLENGE)?]));
    match rx.recv_timeout(GIVE_UP_AFTER) {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(e)) => anyhow::bail!("lyrics verification failed: {e}"),
        Err(_) => anyhow::bail!("lyrics verification timed out"),
    }
}

/// `Bridge.onTurnstileResult(ok, value)`: the token, or why there's none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_gabialonso_ytmrs_Bridge_onTurnstileResult(mut env: JNIEnv, _class: JClass, ok: jboolean, value: JString) {
    let value = rust_string(&mut env, &value);
    if let Some(tx) = PENDING.lock().unwrap().take() {
        let _ = tx.send(if ok != 0 && !value.is_empty() { Ok(value) } else { Err(value) });
    }
}
