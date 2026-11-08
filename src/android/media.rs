//! Android media integration: a `MediaSession` in the Kotlin `PlaybackService` (lock
//! screen, notification, headset and Bluetooth buttons). The service also keeps the
//! process in the foreground so audio goes on with the screen off.
//!
//! Rust sends the song and the play state; the buttons come back through
//! `Bridge.onMediaCommand` as [`MediaControlEvent`]s, the same events souvlaki gives
//! the desktop builds.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use jni::JNIEnv;
use jni::objects::{JClass, JValueOwned};
use jni::sys::{jint, jlong};

use crate::android::{call, jstr};

/// souvlaki's position type, so shared code can match on it the same way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaPosition(pub Duration);

/// The subset of souvlaki's events a MediaSession can send.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaControlEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SetPosition(MediaPosition),
}

type Handler = (Sender<MediaControlEvent>, Arc<dyn Fn() + Send + Sync>);
static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);

static STATUS: OnceLock<Arc<crate::audio::AudioStatus>> = OnceLock::new();

/// The player's position, which the lock screen's progress bar follows.
pub fn set_status(status: Arc<crate::audio::AudioStatus>) {
    let _ = STATUS.set(status);
}

pub struct Media {
    /// The state last sent, the position then, and when: Kotlin extrapolates from it,
    /// so only changes and jumps (a seek, a new song) need sending.
    sent: Option<(bool, Duration, Instant)>,
}

impl Media {
    pub fn new(_hwnd: Option<*mut std::ffi::c_void>, tx: Sender<MediaControlEvent>, wake: impl Fn() + Send + Sync + 'static) -> Option<Self> {
        *HANDLER.lock().unwrap() = Some((tx, Arc::new(wake)));
        Some(Self { sent: None })
    }

    pub fn set_track(&mut self, title: &str, artist: &str, album: Option<&str>, cover: Option<&str>, secs: Option<u32>) {
        let (title, artist, album, cover) = (title.to_owned(), artist.to_owned(), album.unwrap_or_default().to_owned(), cover.unwrap_or_default().to_owned());
        let ms = secs.map_or(-1, |s| s as i64 * 1000);
        call("setTrack", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V", |env| {
            Ok(vec![jstr(env, &title)?, jstr(env, &artist)?, jstr(env, &album)?, jstr(env, &cover)?, JValueOwned::Long(ms)])
        });
    }

    /// Called every frame (and every background tick); only changes reach Kotlin.
    pub fn set_playing(&mut self, playing: bool) {
        // Moving only while sound comes out (not while buffering).
        let (pos, moving) = STATUS.get().map_or((Duration::ZERO, false), |s| (s.position(), s.playing.load(Ordering::Relaxed)));
        if let Some((was, at, when)) = self.sent {
            let expected = if was { at + when.elapsed() } else { at };
            let drift = if pos > expected { pos - expected } else { expected - pos };
            if was == (playing && moving) && drift < Duration::from_millis(1500) {
                return;
            }
        }
        let moving = playing && moving;
        self.sent = Some((moving, pos, Instant::now()));
        call("setState", "(ZZJ)V", |_| Ok(vec![JValueOwned::Bool(playing as u8), JValueOwned::Bool(moving as u8), JValueOwned::Long(pos.as_millis() as i64)]));
    }
}

/// `Bridge.onMediaCommand(cmd, arg)`: a button on the lock screen, notification or a headset.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onMediaCommand(_env: JNIEnv, _class: JClass, cmd: jint, arg: jlong) {
    let ev = match cmd {
        0 => MediaControlEvent::Play,
        1 => MediaControlEvent::Pause,
        2 => MediaControlEvent::Toggle,
        3 => MediaControlEvent::Next,
        4 => MediaControlEvent::Previous,
        5 => MediaControlEvent::Stop,
        6 => MediaControlEvent::SetPosition(MediaPosition(Duration::from_millis(arg.max(0) as u64))),
        _ => return,
    };
    if let Some((tx, wake)) = HANDLER.lock().unwrap().as_ref() {
        let _ = tx.send(ev);
        wake();
    }
}
