//! Android glue: the NativeActivity entry point and the JNI bridge to the Kotlin side
//! (`android/app/src/main/java/.../Bridge.kt`).
//!
//! Rust → Kotlin: static methods on `Bridge`, called through [`call`].
//! Kotlin → Rust: the `Java_..._Bridge_*` functions in the modules that handle them
//! (sign-in in `login.rs`, media buttons in `media.rs`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crossbeam_channel::unbounded;
use winit::platform::android::activity::AndroidApp;
use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};

mod text;

/// The Kotlin class every call goes to.
const BRIDGE: &str = "io.github.usuallygabo.ytmrs.Bridge";

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static CTX: OnceLock<egui::Context> = OnceLock::new();
static APP: OnceLock<Arc<Mutex<crate::ui::App>>> = OnceLock::new();
/// The audio handle, kept so `Bridge.onClosing` can stop playback before the process dies.
static AUDIO: OnceLock<crate::audio::AudioHandle> = OnceLock::new();
static BRIDGE_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// The app's cache directory, next to its files: Android counts what's here as cache,
/// and may clear it when storage runs low.
pub fn cache_dir() -> PathBuf {
    let files = data_dir();
    files.parent().map_or_else(|| files.join("cache"), |p| p.join("cache"))
}

pub fn data_dir() -> PathBuf {
    DATA_DIR.get().cloned().expect("data dir is set in android_main")
}

fn vm() -> Option<JavaVM> {
    let ctx = ndk_context::android_context();
    // SAFETY: the pointer is the process's JavaVM, valid for its whole life.
    unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()
}

/// The `Bridge` class, loaded once through the activity's class loader: `FindClass` on a
/// native thread only sees the system classes.
fn bridge_class(env: &mut JNIEnv) -> jni::errors::Result<&'static GlobalRef> {
    if let Some(c) = BRIDGE_CLASS.get() {
        return Ok(c);
    }
    // SAFETY: ndk-context holds a global reference to the activity for the process's life.
    let activity = unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) };
    let loader = env.call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    let name = env.new_string(BRIDGE)?;
    let class = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?.l()?;
    let global = env.new_global_ref(class)?;
    Ok(BRIDGE_CLASS.get_or_init(|| global))
}

/// Run `f` with this thread attached to the JVM; logs and clears any Java exception.
pub fn with_env<R>(f: impl FnOnce(&mut JNIEnv) -> jni::errors::Result<R>) -> Option<R> {
    let vm = vm()?;
    let mut env = vm.attach_current_thread().map_err(|e| log::error!("jni attach: {e}")).ok()?;
    match f(&mut env) {
        Ok(r) => Some(r),
        Err(e) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            log::error!("jni: {e}");
            None
        }
    }
}

/// Call `Bridge.<method>` (a `@JvmStatic` method). `args` builds the arguments with
/// the env, so strings can be made in place.
pub fn call(method: &str, sig: &str, args: impl FnOnce(&mut JNIEnv) -> jni::errors::Result<Vec<jni::objects::JValueOwned<'static>>>) {
    with_env(|env| {
        let class = bridge_class(env)?;
        // SAFETY: the global ref holds a java.lang.Class.
        let class = unsafe { JClass::from_raw(class.as_obj().as_raw()) };
        let args = args(env)?;
        let args: Vec<JValue> = args.iter().map(|a| a.borrow()).collect();
        env.call_static_method(&class, method, sig, &args)?;
        Ok(())
    });
}

/// A Java string argument for [`call`].
pub fn jstr(env: &mut JNIEnv, s: &str) -> jni::errors::Result<jni::objects::JValueOwned<'static>> {
    let s = env.new_string(s)?;
    // SAFETY: a local reference outlives the call it is passed to (same native frame).
    Ok(jni::objects::JValueOwned::Object(unsafe { JObject::from_raw(s.into_raw()) }))
}

/// A Java string from Kotlin, or "" if it can't be read.
pub fn rust_string(env: &mut JNIEnv, s: &jni::objects::JString) -> String {
    env.get_string(s).map(Into::into).unwrap_or_default()
}

/// Started by NativeActivity (through android-activity) on a thread of its own.
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("ytm-rs")
            .with_max_level(if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info })
            .with_filter(android_logger::FilterBuilder::new().parse("warn,ytm_rs=info").build()),
    );
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));

    let dir = app.internal_data_path().expect("the app has a files dir");
    let _ = std::fs::create_dir_all(&dir);
    let _ = DATA_DIR.set(dir);
    log::info!("ytm-rs {} starting, data in {}", env!("CARGO_PKG_VERSION"), data_dir().display());

    // The lyrics' word glow is a blur worked out on the CPU for each sung word: too slow
    // for a phone. Set before anything reads it (a dev switch in karaoke.rs).
    if std::env::var_os("YTM_LYR_OFF").is_none() {
        // SAFETY: no other thread is running yet.
        unsafe { std::env::set_var("YTM_LYR_OFF", "glow") };
    }
    // Covers used to be kept with the app's data; they're cache now (images.rs).
    let _ = std::fs::remove_dir_all(data_dir().join("thumbs"));
    crate::stream::init_dev_hooks();
    crate::proxy::init(&data_dir());

    let options = eframe::NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Glow,
        persistence_path: Some(data_dir().join("app.ron")),
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("ytm-rs", options, Box::new(|cc| Ok(create(cc)?))) {
        log::error!("eframe: {e}");
    }
}

fn create(cc: &eframe::CreationContext) -> anyhow::Result<Box<dyn eframe::App>> {
    let ctx = cc.egui_ctx.clone();
    crate::theme::install_style(&ctx);
    crate::theme::install_fonts(&ctx, "inter");

    let _ = CTX.set(ctx.clone());
    let wake = || {
        if let Some(c) = CTX.get() {
            c.request_repaint();
        }
        background::wake();
    };

    // Full volume at each start: the phone's volume keys set how loud it is, as for any
    // other player (the slider shows only with the phone held sideways).
    let volume = 1.0f32;
    let log_volume = cc.storage.and_then(|s| s.get_string("log_volume")).as_deref().is_none_or(|v| v == "1");

    let (audio_tx, audio_rx) = unbounded();
    // The phone picks the output (speaker, headphones, Bluetooth) itself.
    let audio = crate::audio::spawn(volume, log_volume, None, audio_tx, wake);

    let (resp_tx, resp_rx) = unbounded();
    let backend = crate::backend::Backend::new(data_dir(), resp_tx, audio.clone(), Arc::new(wake))?;

    egui_extras::install_image_loaders(&ctx);
    ctx.add_bytes_loader(Arc::new(crate::images::HttpImages::new(backend.clone())));

    let (media_tx, media_rx) = unbounded();
    crate::media::set_status(audio.status.clone());
    // Keep a handle so `Bridge.onClosing` can stop playback before the process dies.
    let _ = AUDIO.set(audio.clone());
    let media = crate::media::Media::new(None, media_tx, wake);

    if let Some(gl) = &cc.gl {
        use eframe::glow::HasContext;
        // SAFETY: plain string queries on the current context.
        let (renderer, version) = unsafe { (gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
        log::info!("gl: {renderer} ({version})");
    }
    let bg = cc.gl.as_ref().and_then(|gl| crate::shader::Background::new(gl).map_err(|e| log::warn!("background shader unavailable: {e:#}")).ok());

    let status = audio.status.clone();
    let app = crate::ui::App::new(backend, audio, crate::ui::Channels { resp_rx, audio_rx, media_rx }, media, bg, cc.storage);
    let shell = Shell { app: Arc::new(Mutex::new(app)), clock: Arc::new(Mutex::new((0.0, Instant::now()))) };
    background::spawn(shell.app.clone(), shell.clock.clone(), ctx, status);
    let _ = APP.set(shell.app.clone());
    Ok(Box::new(shell))
}

/// The app, shared with the thread that runs it while there's no window.
struct Shell {
    app: Arc<Mutex<crate::ui::App>>,
    /// egui's clock at the last frame, and when that was: the background thread
    /// carries it on.
    clock: Arc<Mutex<(f64, Instant)>>,
}

impl eframe::App for Shell {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.lock().unwrap().clear_color(visuals)
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Off screen (screen off, another app in front) eframe still runs frames for
        // whatever asked for one, the lyrics above all, though nothing is shown: skip
        // them, so nothing asks for more. The background thread keeps the player going,
        // and `onVisible` asks for a frame on the way back.
        if !background::visible() {
            return;
        }
        *self.clock.lock().unwrap() = (ctx.input(|i| i.time), Instant::now());
        self.app.lock().unwrap().logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if !background::visible() {
            return;
        }
        self.app.lock().unwrap().ui(ui, frame);
        // egui asks for text input while a text field has focus.
        text::follow_focus(ui.ctx().output(|o| o.ime.is_some()));
        // egui-winit has no clipboard on Android: copy through the system's.
        let copied: Vec<String> = ui.ctx().output(|o| o.commands.iter().filter_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None }).collect());
        for t in copied {
            call("copyText", "(Ljava/lang/String;)V", |env| Ok(vec![jstr(env, &t)?]));
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // The window spans the whole screen (edge to edge): lay the UI out clear of the
        // status bar, navigation bar, camera cutout and keyboard.
        if let Some(rect) = raw.screen_rect {
            let ppp = ctx.pixels_per_point();
            let [l, t, r, b] = INSETS.each_ref().map(|v| v.load(Ordering::Relaxed) as f32 / ppp);
            raw.screen_rect = Some(egui::Rect::from_min_max(rect.min + egui::vec2(l, t), rect.max - egui::vec2(r, b)));
        }
        raw.events.extend(text::take());
        // The glyph atlas is as wide as the GPU's biggest texture (16384 on many phones),
        // and every row of it costs that width in memory twice (a copy in RAM, one on the
        // GPU). Narrower, it grows a few rows taller instead.
        raw.max_texture_side = Some(raw.max_texture_side.unwrap_or(4096).min(4096));
        self.app.lock().unwrap().raw_input_hook(ctx, raw);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.lock().unwrap().save(storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        stop_audio();
        self.app.lock().unwrap().on_exit(gl);
    }
}

/// `Bridge.onBack()`: the system's Back, in the app. False sends the app to the background.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onBack(_env: JNIEnv, _class: JClass) -> jni::sys::jboolean {
    let Some(app) = APP.get() else { return 0 };
    let handled = app.lock().unwrap().phone_back();
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
    handled as jni::sys::jboolean
}

/// `Bridge.onClosing()`: stop playback before the process is killed (swiped from Recents,
/// or the activity is destroyed for good).
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onClosing(_env: JNIEnv, _class: JClass) {
    if let Some(audio) = AUDIO.get() {
        audio.send(crate::audio::AudioCmd::Stop);
        log::info!("onClosing: audio stop sent");
    }
}

/// Stop playback: called from `Bridge.onClosing` (Java) and from `Shell::on_exit` (Rust).
pub fn stop_audio() {
    if let Some(audio) = AUDIO.get() {
        audio.send(crate::audio::AudioCmd::Stop);
        log::info!("stop_audio: audio stop sent");
    }
}

/// The safe area's margins in pixels (left, top, right, bottom), from `Bridge.onInsets`.
static INSETS: [std::sync::atomic::AtomicU32; 4] = [const { std::sync::atomic::AtomicU32::new(0) }; 4];

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onInsets(_env: JNIEnv, _class: JClass, left: jni::sys::jint, top: jni::sys::jint, right: jni::sys::jint, bottom: jni::sys::jint) {
    for (slot, v) in INSETS.iter().zip([left, top, right, bottom]) {
        slot.store(v.max(0) as u32, Ordering::Relaxed);
    }
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

/// With the screen off or the app in the background, Android destroys the window and
/// eframe runs no frames, but the player still needs its bookkeeping: the next song when
/// one ends, the lock screen's buttons. This thread runs `App::tick` meanwhile, woken by
/// the same calls that ask for a repaint, and on a timer while a song plays.
mod background {
    use super::*;

    /// Whether the activity is on screen (`Bridge.onVisible`).
    static VISIBLE: AtomicBool = AtomicBool::new(true);
    static SIGNAL: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

    pub fn visible() -> bool {
        VISIBLE.load(Ordering::Relaxed)
    }

    pub fn wake() {
        *SIGNAL.0.lock().unwrap() = true;
        SIGNAL.1.notify_one();
    }

    pub fn spawn(app: Arc<Mutex<crate::ui::App>>, clock: Arc<Mutex<(f64, Instant)>>, ctx: egui::Context, status: Arc<crate::audio::AudioStatus>) {
        std::thread::Builder::new()
            .name("background".into())
            .spawn(move || loop {
                let visible = VISIBLE.load(Ordering::Relaxed);
                let timeout = if visible {
                    Duration::from_secs(3600)
                } else if status.playing.load(Ordering::Relaxed) {
                    Duration::from_millis(500)
                } else {
                    Duration::from_secs(5)
                };
                let (mut woken, _) = SIGNAL.1.wait_timeout_while(SIGNAL.0.lock().unwrap(), timeout, |w| !*w).unwrap();
                *woken = false;
                drop(woken);
                if VISIBLE.load(Ordering::Relaxed) {
                    continue;
                }
                let now = {
                    let (t, at) = *clock.lock().unwrap();
                    t + at.elapsed().as_secs_f64()
                };
                app.lock().unwrap().tick(&ctx, now);
            })
            .expect("a thread for the background player");
    }

    /// `Bridge.onVisible(visible)`, from the activity's onResume / onPause.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onVisible(_env: JNIEnv, _class: JClass, visible: jni::sys::jboolean) {
        VISIBLE.store(visible != 0, Ordering::Relaxed);
        wake();
        if visible != 0 {
            // Frames were skipped while away: draw the current state.
            if let Some(ctx) = CTX.get() {
                ctx.request_repaint();
            }
        } else {
            // Give the memory freed while on screen back to the system (scudo keeps it
            // for reuse otherwise), now that the app may sit in the background for hours.
            unsafe extern "C" {
                /// Bionic's: the libc crate doesn't declare it for Android.
                fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
            }
            const M_PURGE: libc::c_int = -101;
            // SAFETY: a plain call into the allocator, safe from any thread.
            unsafe { mallopt(M_PURGE, 0) };
        }
    }
}
