//! Typing on the phone's keyboard. NativeActivity's own view isn't a text editor, so a
//! keyboard sends it only keys its key map knows: no "ñ" or "á", no words committed from
//! suggestions. Instead the Java `TextInput` view takes the keyboard while an egui text
//! field has focus and passes what's typed here, where it becomes egui events.

use std::sync::Mutex;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jint;

use super::{CTX, call, rust_string};

static PENDING: Mutex<Vec<egui::Event>> = Mutex::new(Vec::new());

fn push(events: impl IntoIterator<Item = egui::Event>) {
    PENDING.lock().unwrap().extend(events);
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

fn key(key: egui::Key) -> [egui::Event; 2] {
    let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
    [ev(true), ev(false)]
}

/// What was typed since the last frame, for the frame's input.
pub fn take() -> Vec<egui::Event> {
    std::mem::take(&mut *PENDING.lock().unwrap())
}

/// Show the keyboard while a text field has focus, hide it when none has. Called after
/// each frame; only changes reach Java.
pub fn follow_focus(wants: bool) {
    static SHOWN: Mutex<bool> = Mutex::new(false);
    let mut shown = SHOWN.lock().unwrap();
    if *shown != wants {
        *shown = wants;
        call("setKeyboard", "(Z)V", |_| Ok(vec![jni::objects::JValueOwned::Bool(wants as u8)]));
    }
}

/// `Bridge.onText(text)`: text the keyboard committed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onText(mut env: JNIEnv, _class: JClass, text: JString) {
    let text = rust_string(&mut env, &text);
    // A newline is the keyboard's Enter.
    let mut events = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            events.extend(key(egui::Key::Enter));
        }
        if !line.is_empty() {
            events.push(egui::Event::Text(line.to_owned()));
        }
    }
    push(events);
}

/// `Bridge.onKey(code)`: 0 = Backspace, 1 = Enter (the keyboard's action button), 2 = Delete.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_usuallygabo_ytmrs_Bridge_onKey(_env: JNIEnv, _class: JClass, code: jint) {
    match code {
        0 => push(key(egui::Key::Backspace)),
        1 => push(key(egui::Key::Enter)),
        2 => push(key(egui::Key::Delete)),
        _ => {}
    }
}
