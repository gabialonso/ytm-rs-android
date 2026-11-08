//! Android: no music videos (CPU decoding costs too much battery). A player fails at
//! once, so the cover art stays.

use std::sync::Arc;

use crate::audio::AudioStatus;
use crate::innertube::VideoStream;

pub struct Frame {
    pub pts: f64,
    pub image: egui::ColorImage,
}

pub const GLOW_SIZE: [usize; 2] = [32, 18];

pub fn glow_colors(_frame: &egui::ColorImage) -> Vec<[f32; 3]> {
    vec![[0.0; 3]; GLOW_SIZE[0] * GLOW_SIZE[1]]
}

pub struct Player;

impl Player {
    pub fn start(_stream: VideoStream, _http: reqwest::Client, _status: Arc<AudioStatus>) -> Self {
        Self
    }

    pub fn take_due(&self) -> Option<Frame> {
        None
    }

    pub fn failed(&self) -> bool {
        true
    }
}
