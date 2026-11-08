//! Android: no Discord Rich Presence (no local Discord IPC to talk to).

#[derive(Clone, Debug, PartialEq)]
pub struct Presence {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub art: Option<String>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
}

pub struct Discord;

impl Discord {
    pub fn spawn() -> Option<Self> {
        None
    }

    pub fn set(&self, _p: Option<Presence>) {}
}
