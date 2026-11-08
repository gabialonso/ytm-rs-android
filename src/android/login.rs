//! "Sign in with Google" on Android: the Kotlin `LoginActivity` shows Google's sign-in
//! page in a WebView and, once it lands on music.youtube.com with a session, hands back
//! the youtube.com `Cookie` header (HttpOnly cookies included, read from CookieManager).

use std::path::PathBuf;
use std::sync::Mutex;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jint;

use crate::android::{call, jstr, rust_string};

#[derive(Clone, Copy)]
pub enum Site {
    Google,
    Spotify,
}

pub enum Outcome {
    Cookie(String),
    Cancelled,
    Failed(String),
}

type Done = Box<dyn FnOnce(Outcome) + Send>;
static PENDING: Mutex<Option<Done>> = Mutex::new(None);

/// Show the sign-in screen; `done` is called exactly once.
pub fn spawn(site: Site, _profile_dir: PathBuf, done: impl FnOnce(Outcome) + Send + 'static) {
    if let Site::Spotify = site {
        done(Outcome::Failed("Spotify sign-in isn't available on Android".into()));
        return;
    }
    // A sign-in still open (shouldn't happen: the button is disabled meanwhile) ends as cancelled.
    if let Some(old) = PENDING.lock().unwrap().replace(Box::new(done)) {
        old(Outcome::Cancelled);
    }
    call("startLogin", "(Ljava/lang/String;)V", |env| {
        Ok(vec![jstr(env, "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F")?])
    });
}

/// Forget the WebView's Google session (on sign-out, and right after signing in: the
/// WebView's copy would go stale and Google treats stale cookies as stolen).
pub fn forget(profile_dir: &std::path::Path) {
    // The Spotify profile has no WebView on Android; its cookies aren't these.
    if profile_dir.file_name().is_some_and(|n| n == "webview") {
        call("clearCookies", "()V", |_| Ok(vec![]));
    }
    let _ = std::fs::remove_dir_all(profile_dir);
}

/// `Bridge.onLoginResult(status, value)`: 0 = signed in (`value` is the Cookie header),
/// 1 = cancelled, 2 = failed (`value` says why).
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onLoginResult(mut env: JNIEnv, _class: JClass, status: jint, value: JString) {
    let value = rust_string(&mut env, &value);
    let outcome = match status {
        0 => Outcome::Cookie(value),
        1 => Outcome::Cancelled,
        _ => Outcome::Failed(value),
    };
    if let Some(done) = PENDING.lock().unwrap().take() {
        // Off the JNI thread: `done` may do network work.
        std::thread::spawn(move || done(outcome));
    }
}
