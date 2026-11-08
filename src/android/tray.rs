//! Android: no tray. `Tray::new` fails, so "Close to tray" never takes effect.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Show,
    Toggle,
    Quit,
}

pub struct Tray;

impl Tray {
    pub fn new(_icon: &crate::appicon::Variant, _wake: impl Fn() + Send + Sync + Clone + 'static) -> anyhow::Result<Self> {
        anyhow::bail!("no tray on Android")
    }

    pub fn set_shown(&mut self, _shown: bool) {}
}

pub fn take() -> Vec<Cmd> {
    Vec::new()
}
