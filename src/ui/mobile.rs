//! The phone layout (the Android build held upright): the same pages, rows, cards, now
//! playing and settings as the desktop window, in a phone's chrome. A mini player and
//! tabs along the bottom take the sidebar's and the player bar's place; in now playing
//! the bottom area holds its seek bar and buttons instead.
//!
//! Everything phone-only lives here, so the shared code only asks [`phone`] at the few
//! places where the two layouts part.

use std::sync::atomic::{AtomicU32, Ordering};

use egui::{Align2, Color32, Rect, Sense, Ui, Vec2, pos2, vec2};

use super::widgets::{self, icon_at, named, paint_cover, text_at};
use super::{Action, App, LibTab, NpTab, Page, Repeat, fmt_time, pages, shell};
use crate::theme::{self, icon};

/// Wider than this (a tablet, a phone turned sideways) the desktop layout fits.
const PHONE_MAX_W: f32 = 600.0;
const MINI_H: f32 = 60.0;
const TABS_H: f32 = 60.0;
/// Now playing's seek bar, buttons and their bottom row.
const CONTROLS_H: f32 = 216.0;

/// Whether to lay the app out for a phone: the Android build, upright.
pub(super) fn phone(ctx: &egui::Context) -> bool {
    cfg!(target_os = "android") && ctx.content_rect().width() < PHONE_MAX_W
}

/// The bottom area's height while the phone layout shows (f32 bits; 0 when it doesn't).
static BAR_H: AtomicU32 = AtomicU32::new(0);

/// For `App::bar_height`: now playing ends where the bottom area starts.
pub(super) fn bar_height() -> Option<f32> {
    match BAR_H.load(Ordering::Relaxed) {
        0 => None,
        bits => Some(f32::from_bits(bits)),
    }
}

/// The window's chrome and the page, for a phone. `covered`: now playing fills the
/// screen, so the page under it isn't drawn.
pub(super) fn layout(app: &mut App, ui: &mut Ui, now: f64, covered: bool) {
    let ctx = ui.ctx().clone();
    let np = theme::anim_bool(&ctx, "np-open", app.np_open && app.current().is_some(), 0.42);
    let h = TABS_H + if app.current().is_some() { MINI_H } else { 0.0 };
    BAR_H.store(h.to_bits(), Ordering::Relaxed);

    let bar_alpha = app.look().app.bar_alpha.clamp(0.0, 1.0);
    egui::Panel::bottom("player")
        .resizable(false)
        .exact_size(h)
        .frame(egui::Frame::new().fill(theme::panel(bar_alpha)))
        .show_separator_line(false)
        .show(ui, |ui| {
            let r = ui.max_rect();
            let mut y = r.top();
            if app.current().is_some() {
                mini_player(app, ui, Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), MINI_H)), now);
                y += MINI_H;
            }
            tabs(app, ui, Rect::from_min_max(pos2(r.left(), y), pos2(r.right(), y + TABS_H)));
        });
    // Now playing's controls ride up with it, over its backdrop.
    if np >= 1.0 {
        // A swipe down closes it: from its header, or anywhere over the artwork alone (the
        // lyrics and Up next scroll).
        let screen = ctx.content_rect();
        let from = if app.np_tab == NpTab::Art { screen.bottom() - CONTROLS_H } else { screen.top() + 110.0 };
        let area = Rect::from_min_max(screen.min, pos2(screen.right(), from));
        if swipe(ui, egui::Id::unique("np-swipe"), area).is_some_and(|d| d.y > 90.0 && d.y > d.x.abs() * 1.5) {
            app.np_open = false;
        }
    }
    if np > 0.001 {
        let screen = ctx.content_rect();
        let r = Rect::from_min_max(pos2(screen.left(), screen.bottom() - CONTROLS_H), screen.max).translate(vec2(0.0, screen.height() * (1.0 - np)));
        egui::Area::new(egui::Id::unique("np-controls")).order(egui::Order::Foreground).fixed_pos(r.min).show(&ctx, |ui| {
            ui.set_min_size(r.size());
            controls(app, ui, r, now);
        });
    }
    if covered {
        return;
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 0, top: 10, bottom: 0 }))
        .show(ui, |ui| {
            ui.scope(|ui| {
                ui.set_max_width(ui.available_width() - 12.0);
                shell::top_bar(app, ui);
            });
            ui.add_space(6.0);
            pages::page(app, ui, now);
        });
}

/// Where now playing lays out its artwork and lyrics: above the controls on a phone,
/// with narrower side margins.
pub(super) fn np_body(ctx: &egui::Context, r: Rect) -> Rect {
    if !phone(ctx) {
        return r;
    }
    Rect::from_min_max(pos2(r.left() - 12.0, r.top()), pos2(r.right() + 12.0, r.bottom() + 20.0 - CONTROLS_H))
}

/// The playing song along the top of the bottom area: tap it for now playing.
fn mini_player(app: &mut App, ui: &mut Ui, r: Rect, now: f64) {
    let Some(t) = app.current().cloned() else { return };
    let status = app.audio.status.clone();
    let total = status.duration().map(|d| d.as_secs_f32()).or(t.duration.map(|d| d as f32)).unwrap_or(0.0);
    let pos = status.position_smooth().as_secs_f32().min(total.max(0.0));

    // The whole strip opens now playing (a tap or a swipe up); its buttons take their own
    // taps first.
    let open = named(ui.interact(r, ui.scope_id().with("mini-open"), Sense::click()), "Open now playing");
    let up = swipe(ui, egui::Id::unique("mini-swipe"), r).is_some_and(|d| d.y < -40.0 && d.y.abs() > d.x.abs());
    if open.clicked() || up {
        app.actions.push(Action::OpenNowPlaying(true));
    }

    // Progress: a hairline along the top.
    let line = Rect::from_min_size(r.min, vec2(r.width(), 2.0));
    ui.painter().rect_filled(line, 0.0, theme::glass(0.12));
    if total > 0.0 {
        ui.painter().rect_filled(Rect::from_min_size(line.min, vec2(line.width() * pos / total, 2.0)), 0.0, app.accent_color());
    }

    let cy = r.center().y + 1.0;
    let art = Rect::from_center_size(pos2(r.left() + 16.0 + 22.0, cy), Vec2::splat(44.0));
    paint_cover(ui, art, &t.thumbs, theme::thumb_radius(), 1.0);

    // Right: play / pause, next.
    let next = Rect::from_center_size(pos2(r.right() - 16.0 - 20.0, cy), Vec2::splat(40.0));
    let play = Rect::from_center_size(pos2(next.left() - 26.0, cy), Vec2::splat(44.0));
    if play_button(app, ui, play, 19.0, now).clicked() {
        app.toggle();
    }
    let mut nui = ui.new_child(egui::UiBuilder::new().max_rect(next));
    if widgets::icon_button(&mut nui, icon::SKIP_FORWARD, 20.0, "Next", false).clicked() {
        app.next();
    }

    let x = art.right() + 12.0;
    let w = (play.left() - 8.0 - x).max(40.0);
    text_at(ui, pos2(x, cy - 19.0), &t.title, theme::semibold(15.0), theme::text(), w);
    text_at(ui, pos2(x, cy + 1.0), &t.artist_line(), theme::regular(13.0), theme::text_dim(), w);
}

/// Home, Explore and Library, along the bottom edge.
fn tabs(app: &mut App, ui: &mut Ui, r: Rect) {
    let accent = app.accent_color();
    let items: [(char, &str, Page, bool); 4] = [
        (icon::HOUSE, "Home", Page::Home, app.page == Page::Home),
        (icon::COMPASS, "Explore", Page::Explore, app.page == Page::Explore),
        (icon::LIBRARY, "Library", Page::Library(LibTab::Playlists), matches!(app.page, Page::Library(_))),
        (icon::SETTINGS, "Settings", Page::Settings, app.page == Page::Settings),
    ];
    let w = r.width() / items.len() as f32;
    for (i, (glyph, label, page, selected)) in items.into_iter().enumerate() {
        let cell = Rect::from_min_size(pos2(r.left() + w * i as f32, r.top()), vec2(w, r.height()));
        let resp = named(ui.interact(cell, ui.scope_id().with(("tab", i)), Sense::click()), label);
        let s = theme::anim_bool(ui.ctx(), resp.id.with("s"), selected, 0.2);
        let icon_c = pos2(cell.center().x, cell.top() + 22.0);
        if s > 0.0 {
            // The selected tab's pill, as the sidebar marks the page it's on.
            let pill = Rect::from_center_size(icon_c, vec2(56.0 * (0.6 + 0.4 * s), 30.0));
            ui.painter().rect_filled(pill, 15.0, theme::with_alpha(theme::lerp_color(theme::glass(0.14), accent, 0.25), s));
        }
        let fg = theme::lerp_color(theme::text_dim(), theme::text(), s);
        let press = if resp.is_pointer_button_down_on() { 0.9 } else { 1.0 };
        icon_at(ui, icon_c, glyph, 20.0 * press, fg);
        let font = if selected { theme::semibold(12.0) } else { theme::regular(12.0) };
        ui.painter().text(pos2(cell.center().x, cell.top() + 46.0), Align2::CENTER_CENTER, label, font, fg);
        if resp.clicked() {
            app.actions.push(Action::Go(page));
        }
    }
}

/// Now playing's controls, under its artwork or lyrics: seek bar, transport, and a row
/// with Like, Lyrics and Up next.
fn controls(app: &mut App, ui: &mut Ui, r: Rect, now: f64) {
    let r = r.shrink2(vec2(24.0, 0.0));
    let status = app.audio.status.clone();
    let track = app.current().cloned();
    let total = status.duration().map(|d| d.as_secs_f32()).or(track.as_ref().and_then(|t| t.duration).map(|d| d as f32)).unwrap_or(0.0);
    let pos = app.seek_drag.unwrap_or(status.position_smooth().as_secs_f32()).min(total.max(0.0));
    let accent = app.accent_color();

    // Seek bar: a wide strip to grab, a thin line with a knob, the times under it.
    let line_y = r.top() + 14.0;
    let hit = Rect::from_min_max(pos2(r.left() - 8.0, line_y - 16.0), pos2(r.right() + 8.0, line_y + 16.0));
    let resp = ui.interact(hit, ui.scope_id().with("np-seek"), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::slider(total > 0.0, pos as f64, "Seek"));
    let held = theme::anim_bool(ui.ctx(), "np-seek-held", resp.dragged(), 0.12);
    let thick = 4.0 + 2.0 * held;
    let line = Rect::from_center_size(pos2(r.center().x, line_y), vec2(r.width(), thick));
    ui.painter().rect_filled(line, thick / 2.0, Color32::from_white_alpha(46));
    let frac = if total > 0.0 { pos / total } else { 0.0 };
    ui.painter().rect_filled(Rect::from_min_size(line.min, vec2(line.width() * frac, thick)), thick / 2.0, accent);
    if total > 0.0 {
        ui.painter().circle_filled(pos2(line.left() + line.width() * frac, line_y), 7.0 + 3.0 * held, Color32::WHITE);
        if let Some(p) = resp.interact_pointer_pos() {
            let t = ((p.x - line.left()) / line.width()).clamp(0.0, 1.0) * total;
            if resp.dragged() {
                app.seek_drag = Some(t);
            }
            if resp.drag_stopped() || resp.clicked() {
                app.seek_drag = None;
                app.actions.push(Action::Seek(t));
            }
        }
    }
    let dim = Color32::from_white_alpha(170);
    let times_y = line_y + 18.0;
    ui.painter().text(pos2(r.left(), times_y), Align2::LEFT_CENTER, fmt_time(pos as u32), theme::regular(12.5), dim);
    if total > 0.0 {
        ui.painter().text(pos2(r.right(), times_y), Align2::RIGHT_CENTER, fmt_time(total as u32), theme::regular(12.5), dim);
    }

    // Transport.
    let cy = r.top() + 86.0;
    let c = r.center().x;
    let step = (r.width() / 4.6).min(84.0);
    let button = |ui: &mut Ui, x: f32, glyph: char, name: &str, active: bool, size: f32| -> bool {
        let rect = Rect::from_center_size(pos2(x, cy), Vec2::splat(size + 22.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        let clicked = widgets::icon_button(&mut child, glyph, size, name, active).clicked();
        if active {
            ui.painter().circle_filled(rect.center_bottom() + vec2(0.0, -2.0), 2.0, accent);
        }
        clicked
    };
    if button(ui, c - 2.0 * step, icon::SHUFFLE, "Shuffle up next", false, 20.0) {
        app.shuffle_upcoming();
    }
    if button(ui, c - step, icon::SKIP_BACK, "Previous", false, 26.0) {
        app.prev();
    }
    if play_button(app, ui, Rect::from_center_size(pos2(c, cy), Vec2::splat(68.0)), 26.0, now).clicked() {
        app.toggle();
    }
    if button(ui, c + step, icon::SKIP_FORWARD, "Next", false, 26.0) {
        app.next();
    }
    let (rep_icon, rep_on) = match app.queue.repeat {
        Repeat::Off => (icon::REPEAT, false),
        Repeat::All => (icon::REPEAT, true),
        Repeat::One => (icon::REPEAT_1, true),
    };
    if button(ui, c + 2.0 * step, rep_icon, "Repeat", rep_on, 20.0) {
        app.queue.repeat = match app.queue.repeat {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
    }

    // Up next and Lyrics (the side panel's two views), as two tabs side by side, along
    // the bottom with room between them and the transport.
    let by = r.bottom() - 30.0;
    let no_lyrics = app.no_lyrics();
    let gap = 10.0;
    let w = (r.width() - gap) / 2.0;
    for (k, (tab, glyph, label)) in [(NpTab::UpNext, icon::LIST_MUSIC, "Up next"), (NpTab::Lyrics, icon::MIC_VOCAL, "Lyrics")].into_iter().enumerate() {
        let on = app.np_tab == tab && !(tab == NpTab::Lyrics && no_lyrics);
        let chip = Rect::from_min_size(pos2(r.left() + k as f32 * (w + gap), by - 18.0), vec2(w, 36.0));
        let resp = named(ui.interact(chip, ui.scope_id().with(("np-tab", label)), Sense::click()), label);
        let s = theme::anim_bool(ui.ctx(), resp.id.with("s"), on, 0.2);
        let off = tab == NpTab::Lyrics && no_lyrics;
        let a = if off { 0.4 } else { 1.0 };
        ui.painter().rect_filled(chip, 18.0, theme::with_alpha(theme::lerp_color(Color32::from_white_alpha(26), Color32::WHITE, s), a));
        let fg = theme::with_alpha(theme::lerp_color(Color32::WHITE, Color32::BLACK, s), a);
        let g = widgets::galley(ui, label, theme::semibold(13.5), fg, 200.0);
        let x = chip.center().x - (g.size().x + 22.0) / 2.0;
        icon_at(ui, pos2(x + 7.0, by), glyph, 15.0, fg);
        ui.painter().galley(pos2(x + 22.0, by - g.size().y / 2.0), g, fg);
        if resp.clicked() && !off {
            // Tapping the view that's showing goes back to the artwork alone.
            app.np_tab = if app.np_tab == tab { NpTab::Art } else { tab };
        }
    }
}

/// The heart for what's heard, `size` big, centred in `r`; tap to like or unlike.
fn like_button(app: &mut App, ui: &mut Ui, r: Rect, size: f32) {
    let liked = app.heard_liked();
    let heard_id = app.playing_track().map(|t| t.id.clone()).unwrap_or_default();
    let resp = named(ui.interact(r, ui.scope_id().with(("np-like", r.min.x as i32, r.min.y as i32)), Sense::click()), if liked { "Remove like" } else { "Like" });
    let pop = theme::anim_bool(ui.ctx(), ("liked", &heard_id), liked, 0.3);
    let size = size * (1.0 + 0.25 * (pop * std::f32::consts::PI).sin());
    let dim = Color32::from_white_alpha(170);
    widgets::heart(ui, r.center(), size, pop, theme::lerp_color(dim, theme::RED, pop), theme::RED);
    if resp.clicked() {
        app.actions.push(Action::LikeHeard(!liked));
    }
}

/// Now playing's artwork view on a phone, laid out in `col`, as a music player has it:
/// the cover across the width, then the title with the heart beside it, and the artists.
/// False off a phone, for the desktop's own.
pub(super) fn np_cover(app: &mut App, ui: &mut Ui, col: Rect, track: &crate::model::Track) -> bool {
    if !phone(ui.ctx()) {
        return false;
    }
    let cs = &app.look().cover;
    // In line with the seek bar under it.
    let col = col.shrink2(vec2(4.0, 0.0));
    // Room under the cover for the title and artists.
    const TEXT_H: f32 = 76.0;
    let side = col.width().min(col.height() - TEXT_H).max(120.0);
    let top = col.top() + ((col.height() - side - TEXT_H) * 0.4).max(0.0);
    let art = Rect::from_min_size(pos2(col.center().x - side / 2.0, top), Vec2::splat(side));
    let radius = cs.radius.clamp(0.0, side / 2.0).max(8.0);
    widgets::paint_cover_px(ui, art, &track.thumbs, radius, 1.0, (side * ui.ctx().pixels_per_point()) as u32);
    let title_y = art.bottom() + 20.0;
    let text_w = side - 48.0;
    text_at(ui, pos2(art.left(), title_y), &track.title, theme::bold(22.0), Color32::WHITE, text_w);
    let line = track.artist_line();
    widgets::linked_text(ui, pos2(art.left(), title_y + 32.0), &line, &track.artists, theme::regular(15.0), theme::text_dim(), text_w, &mut app.actions);
    like_button(app, ui, Rect::from_center_size(pos2(art.right() - 18.0, title_y + 24.0), Vec2::splat(44.0)), 24.0);
    true
}

/// While Lyrics or Up next fill now playing, the song stays in sight in its header, between
/// the close button and the menu: a small cover, the title and artists, and the heart.
pub(super) fn np_header(app: &mut App, ui: &mut Ui, header: Rect, track: &crate::model::Track) {
    if !phone(ui.ctx()) {
        return;
    }
    let panel = app.np_tab == NpTab::UpNext || (app.np_tab == NpTab::Lyrics && !app.no_lyrics());
    let v = theme::anim_bool(ui.ctx(), "np-header-song", panel, 0.25);
    if v <= 0.0 {
        return;
    }
    // Clear of the close button on the left and the ⋯ menu on the right.
    let r = Rect::from_min_max(pos2(header.left() + 50.0, header.top()), pos2(header.right() - 50.0, header.bottom()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
    child.multiply_opacity(v);
    let ui = &mut child;
    let art = Rect::from_min_size(pos2(r.left(), r.center().y - 20.0), Vec2::splat(40.0));
    widgets::paint_cover_px(ui, art, &track.thumbs, 6.0, 1.0, (40.0 * ui.ctx().pixels_per_point()) as u32);
    let heart = Rect::from_center_size(pos2(r.right() - 20.0, r.center().y), Vec2::splat(40.0));
    let text_w = (heart.left() - art.right() - 16.0).max(40.0);
    text_at(ui, pos2(art.right() + 10.0, r.center().y - 19.0), &track.title, theme::semibold(15.0), Color32::WHITE, text_w);
    text_at(ui, pos2(art.right() + 10.0, r.center().y + 1.0), &track.artist_line(), theme::regular(13.0), theme::text_dim(), text_w);
    like_button(app, ui, heart, 20.0);
}

/// The round play / pause button (a spinner while the song loads), as on the player bar.
fn play_button(app: &App, ui: &mut Ui, r: Rect, glyph_r: f32, now: f64) -> egui::Response {
    let playing = app.playing();
    let resp = named(ui.interact(r, ui.scope_id().with(("play", r.min.x as i32, r.min.y as i32)), Sense::click()), if playing { "Pause" } else { "Play" });
    let accent = app.accent_color();
    let (fill, glyph) = if app.accent_controls {
        (accent, widgets::contrast(accent))
    } else {
        (if theme::light() { theme::text() } else { Color32::WHITE }, theme::on_text())
    };
    let scale = if resp.is_pointer_button_down_on() { 0.92 } else { 1.0 };
    let center = r.center();
    ui.painter().circle_filled(center, r.width() / 2.0 * scale, fill);
    let morph = theme::anim_bool(ui.ctx(), ("play-morph", r.min.y as i32), playing || app.buffering, 0.18);
    if app.buffering {
        let a = now as f32 * 6.0;
        ui.painter().add(egui::Shape::Path(egui::epaint::PathShape::line(
            (0..20).map(|i| {
                let t = a + i as f32 * 0.22;
                center + vec2(t.cos(), t.sin()) * glyph_r * 0.5
            })
            .collect(),
            egui::epaint::PathStroke::new(2.5, glyph),
        )));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    } else if morph > 0.5 {
        widgets::pause_bars(ui, center, glyph_r * 0.42 * (2.0 * morph - 1.0).max(0.4), glyph);
    } else {
        widgets::play_triangle(ui, center + vec2(glyph_r * 0.08, 0.0), glyph_r * 0.45 * (1.0 - 2.0 * morph).max(0.4), glyph);
    }
    resp
}

// ------------------------------------------------------------------ pages

/// Width of a card in a grid with `gap` between cards: two columns on a phone, `desktop`
/// elsewhere.
/// A carousel column's width (Quick picks): the whole width on a phone, so no column
/// shows cut off at the edge.
pub(super) fn column_width(ui: &Ui, desktop: f32) -> f32 {
    if phone(ui.ctx()) { ui.available_width().floor() } else { desktop }
}

/// Frame pacing on a phone, in place of the desktop's (true when it's set): each frame
/// lays the whole UI out again, the main cost on the battery. The seek bar and the mini
/// player's line move a fraction of a point a second, so 4 fps reads as smooth; the
/// playing row's bars get 30, and the synced lyrics ask for their own while they move.
pub(super) fn pace(app: &App, ctx: &egui::Context) -> bool {
    if !phone(ctx) || app.window_hidden {
        return false;
    }
    if app.playing() || app.buffering {
        let lyrics = app.np_open && app.np_tab == NpTab::Lyrics && !app.no_lyrics();
        let ms = if lyrics || widgets::equalizer_shown(ctx) { 33 } else { 250 };
        ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }
    true
}

/// The synced lyrics are moving: the next frame. On a phone at most 60 a second, not
/// one for each refresh of a 120 Hz screen.
pub(super) fn lyrics_frame(ctx: &egui::Context) {
    if phone(ctx) {
        ctx.request_repaint_after(std::time::Duration::from_millis(15));
    } else {
        ctx.request_repaint();
    }
}

pub(super) fn card_width(ui: &Ui, desktop: f32, gap: f32) -> f32 {
    if phone(ui.ctx()) { ((ui.available_width() - gap) / 2.0).floor().min(desktop + 40.0) } else { desktop }
}

/// Settings (sections and rows, by title) that do nothing in the Android build: the
/// window, tray and taskbar, updates, files, Discord and the like, video, and themes.
pub(super) fn hidden_setting(title: &str) -> bool {
    const HIDDEN: &[&str] = &[
        "Window",
        "App icon",
        "Updates",
        "Downloads",
        "Keyboard shortcuts",
        "Connections",
        "Import",
        "Themes",
        "Output device",
        "Show music videos",
        "Ambient light",
        "Windows proxy",
        "Zoom",
        "Logarithmic volume",
        "Animated background",
        "Pulse with the music",
    ];
    cfg!(target_os = "android") && HIDDEN.contains(&title)
}

/// A settings row on a phone: the control beside the text when it leaves the text room
/// (a switch), else under it (a button, a list to pick from). Its width is the one it
/// took last frame.
pub(super) fn setting_row(ui: &mut Ui, title: &str, desc: Option<&str>, control: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    let id = ui.scope_id().with(("setting-control", title));
    let ctl_w: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(56.0);
    let beside = w - ctl_w - 16.0 >= 170.0;
    let text_w = if beside { w - ctl_w - 16.0 } else { w };
    let title_g = widgets::galley(ui, title, theme::semibold(14.5), theme::text(), text_w);
    let desc_g = desc.map(|d| widgets::galley_wrapped(ui, d, theme::regular(12.5), theme::text_dim(), text_w, 4));
    let text_h = title_g.size().y + desc_g.as_ref().map_or(0.0, |g| g.size().y + 3.0);
    let ctl_h = 40.0;
    let h = if beside { (text_h + 26.0).max(58.0) } else { text_h + 26.0 + ctl_h + 8.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let fade = if ui.is_enabled() { 1.0 } else { 0.45 };
    let top = if beside { rect.center().y - text_h / 2.0 } else { rect.top() + 13.0 };
    ui.painter().galley(pos2(rect.left(), top), title_g.clone(), theme::with_alpha(theme::text(), fade));
    if let Some(g) = desc_g {
        ui.painter().galley(pos2(rect.left(), top + title_g.size().y + 3.0), g, theme::with_alpha(theme::text_dim(), fade));
    }
    let ctl = if beside {
        Rect::from_min_max(pos2(rect.right() - ctl_w, rect.top()), rect.max)
    } else {
        Rect::from_min_max(pos2(rect.left(), rect.bottom() - 13.0 - ctl_h), pos2(rect.right(), rect.bottom() - 13.0))
    };
    let layout = if beside { egui::Layout::right_to_left(egui::Align::Center) } else { egui::Layout::left_to_right(egui::Align::Center) };
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(ctl).layout(layout));
    control(&mut cui);
    let used = cui.min_rect().width();
    if (used - ctl_w).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, used));
        ui.ctx().request_repaint();
    }
}

/// Side by side on a wider screen, one under the other on a phone.
pub(super) fn row_or_column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    if phone(ui.ctx()) { ui.vertical(add) } else { ui.horizontal_top(add) }
}

/// How far a finger moves on a carousel before the swipe counts as sideways or up and down.
const SLOP: f32 = 8.0;
/// How far ahead a sideways flick throws a carousel: its speed times this many seconds,
/// at most a screen's width, then it settles on the nearest card.
const FLING: f32 = 0.2;
/// How long a carousel takes to settle on a card once the finger lets go.
const SETTLE: f64 = 0.28;

/// What a touch that landed on a carousel turned out to be.
#[derive(Clone, Copy)]
enum Swipe {
    /// Too short to tell yet: the carousel stays at this offset.
    Undecided(f32),
    /// Sideways: the carousel follows the finger.
    Sideways,
    /// Up or down: handed to the page, while the carousel stays at this offset.
    Page(f32),
    /// Let go after a sideways swipe: gliding from one offset to a card's, since a time.
    Settle(f32, f32, f64),
}

fn page_drag_key() -> egui::Id {
    egui::Id::unique("mobile-page-drag")
}

/// Remembers the page's scroll area (built in `ui` with this `id_salt`), so a swipe up
/// or down that lands on a carousel can be handed to it. `back`: how many pages Back
/// goes through. A page opened anew starts at its top with its carousels at their start;
/// one gone back to is where it was left.
pub(super) fn page_scroll(ui: &Ui, salt: impl egui::AsIdSalt, back: usize) {
    if !phone(ui.ctx()) {
        return;
    }
    let state = ui.make_persistent_id(egui::IdSalt::new(salt));
    // The ID ScrollArea gives its drag-to-scroll background.
    let id = state.with("area");
    let last_key = egui::Id::unique("mobile-last-page");
    let last = ui.ctx().data(|d| d.get_temp::<(egui::Id, usize)>(last_key));
    if let Some((last_id, last_back)) = last
        && last_id != id
        && back > last_back
    {
        ui.ctx().data_mut(|d| {
            d.remove::<egui::scroll_area::State>(state);
            *d.get_temp_mut_or_default::<u32>(id.with("visit")) += 1;
        });
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(last_key, (id, back));
        d.insert_temp(page_drag_key(), id);
    });
}

/// A carousel's ID, `id` on its own page and visit: each artist, each search keeps its
/// carousels' offsets until it's opened anew, rather than opening at the last one's.
pub(super) fn carousel_id(ctx: &egui::Context, id: egui::Id) -> egui::Id {
    match ctx.data(|d| d.get_temp::<egui::Id>(page_drag_key())) {
        Some(page) if phone(ctx) => id.with((page, ctx.data(|d| d.get_temp::<u32>(page.with("visit")).unwrap_or(0)))),
        _ => id,
    }
}

/// The offset to keep the carousel with scroll area `area` at, while its swipe isn't
/// sideways or it settles on a card.
pub(super) fn carousel_hold(ui: &mut Ui, area: egui::Id) -> Option<f32> {
    if !phone(ui.ctx()) {
        return None;
    }
    // No coasting of egui's own: letting go settles on a card instead (`carousel_touch`).
    ui.spacing_mut().scroll.kinetic.decay_time = 0.0;
    match ui.data(|d| d.get_temp::<Swipe>(area.with("swipe")))? {
        Swipe::Undecided(x) | Swipe::Page(x) => Some(x),
        Swipe::Settle(from, to, since) => {
            let t = ((ui.input(|i| i.time) - since) / SETTLE).clamp(0.0, 1.0) as f32;
            Some(from + (to - from) * theme::ease_out(t))
        }
        Swipe::Sideways => None,
    }
}

/// After the carousel with scroll area `area` is shown, scrolled to `offset`, with cards
/// `step` apart (0: no cards to settle on), its last one ending at `end`, `view` wide. egui gives the whole drag to the innermost
/// scroll area, which here only moves sideways; once a swipe that landed on the carousel
/// goes up or down, the page takes the drag over, with its own glide when the finger
/// lets go. A sideways one ends on a whole card.
pub(super) fn carousel_touch(ui: &Ui, area: egui::Id, offset: f32, step: f32, end: f32, view: f32) {
    if !phone(ui.ctx()) {
        return;
    }
    let ctx = ui.ctx();
    let key = area.with("swipe");
    let state = ctx.data(|d| d.get_temp::<Swipe>(key));
    let (down, now) = ctx.input(|i| (i.pointer.primary_down(), i.time));
    let set = |s: Swipe| {
        ctx.data_mut(|d| d.insert_temp(key, s));
        ctx.request_repaint();
    };
    match state {
        None | Some(Swipe::Settle(..)) if down && ctx.is_being_dragged(area.with("area")) => {
            // Just landed: undo this frame's move and wait for the direction.
            set(Swipe::Undecided(offset + ctx.input(|i| i.pointer.delta().x)));
        }
        Some(Swipe::Undecided(x)) if down => {
            let Some(moved) = ctx.input(|i| Some(i.pointer.latest_pos()? - i.pointer.press_origin()?)) else { return };
            if moved.length() < SLOP {
                return;
            }
            match ctx.data(|d| d.get_temp::<egui::Id>(page_drag_key())) {
                // Leaning towards up and down: a page scroll that crosses a carousel stays one.
                Some(page) if moved.y.abs() > moved.x.abs() * 0.8 => {
                    ctx.set_dragged_id(page);
                    set(Swipe::Page(x));
                }
                _ => set(Swipe::Sideways),
            }
        }
        Some(Swipe::Sideways) if !down => {
            // The offset runs against the finger.
            let throw = (-ctx.input(|i| i.pointer.velocity().x) * FLING).clamp(-view, view);
            // Stops with the last card at the right edge, not a page's worth of room after it.
            let max = (end - view).max(0.0);
            let to = if step > 0.0 { ((offset + throw) / step).round() * step } else { offset + throw };
            set(Swipe::Settle(offset, to.clamp(0.0, max), now));
        }
        Some(Swipe::Settle(_, _, since)) if now - since < SETTLE => ctx.request_repaint(),
        Some(Swipe::Undecided(_) | Swipe::Page(_) | Swipe::Settle(..)) if !down => ctx.data_mut(|d| d.remove::<Swipe>(key)),
        _ => {}
    }
}

/// A row of chips: wrapped onto more lines on a wider screen; on a phone, one line that
/// scrolls sideways like a carousel.
pub(super) fn wrapped_or_scroll<R>(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    if !phone(ui.ctx()) {
        return ui.horizontal_wrapped(add).inner;
    }
    let sid = carousel_id(ui.ctx(), egui::Id::unique(("chip-row", id)));
    let state_id = ui.make_persistent_id(egui::IdSalt::new(sid));
    let mut area = egui::ScrollArea::horizontal().id_salt(sid).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(x) = carousel_hold(ui, state_id) {
        area = area.horizontal_scroll_offset(x);
    }
    let out = area.show(ui, |ui| ui.horizontal(add).inner);
    carousel_touch(ui, out.id, out.state.unclamped_offset().x, 0.0, out.content_size.x, out.inner_rect.width());
    out.inner
}

/// Whether to show a hover effect: never on a touch screen, where it would light up
/// whatever a finger passes over while scrolling, and stay after a tap.
pub(super) fn hover(on: bool) -> bool {
    on && !cfg!(target_os = "android")
}

/// The lyrics' look on Android: no blurred lines. Each blur is a Gaussian worked out on
/// the CPU, again on every frame a line's blur or size changes, too slow at a phone's
/// pixel density. (The words' glow is off too: `YTM_LYR_OFF`, set in `android_main`.)
pub(super) fn lyric_style(s: &'static crate::preset::LyricStyle) -> std::borrow::Cow<'static, crate::preset::LyricStyle> {
    if !cfg!(target_os = "android") {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut s = s.clone();
    s.blur = 0.0;
    if let Some(css) = &mut s.css {
        for v in [&mut css.past, &mut css.upcoming].into_iter().flatten() {
            v[1] = 0.0;
        }
    }
    std::borrow::Cow::Owned(s)
}

/// Room for a page's cover (an album's, a playlist's): `side` square on a wider screen;
/// on a phone, bigger and centred above the title.
pub(super) fn cover_slot(ui: &mut Ui, side: f32) -> Rect {
    if !phone(ui.ctx()) {
        return ui.allocate_exact_size(Vec2::splat(side), Sense::hover()).0;
    }
    let w = ui.available_width();
    let s = (w * 0.68).clamp(side, 300.0);
    let (row, _) = ui.allocate_exact_size(vec2(w, s), Sense::hover());
    Rect::from_center_size(row.center(), Vec2::splat(s))
}

/// A page heading's size: smaller on a phone, where the desktop size takes a line a word.
pub(super) fn title_size(ctx: &egui::Context, desktop: f32) -> f32 {
    if phone(ctx) { (desktop * 0.7).max(24.0) } else { desktop }
}

impl App {
    /// The phone's Back: closes a dialog or now playing, else goes back a page. False when
    /// there's nowhere back to go (the app then goes to the background, as Home does).
    pub fn phone_back(&mut self) -> bool {
        if self.dialog.is_some() {
            self.dialog = None;
        } else if self.np_open || !self.back.is_empty() {
            self.nav_back();
        } else {
            return false;
        }
        true
    }
}

/// A swipe that started in `rect`: how far the finger went, on the frame it lets go.
fn swipe(ui: &Ui, id: egui::Id, rect: Rect) -> Option<Vec2> {
    let (down, pressed, pos) = ui.input(|i| (i.pointer.primary_down(), i.pointer.primary_pressed(), i.pointer.latest_pos()));
    // Where it started and where the finger was last seen: once it lifts, a touch
    // screen has no pointer position any more.
    let key = id.with("from");
    if pressed && let Some(p) = pos.filter(|p| rect.contains(*p)) {
        ui.data_mut(|d| d.insert_temp(key, (p, p)));
    }
    if down {
        if let Some(p) = pos {
            ui.data_mut(|d| {
                if let Some((from, _)) = d.get_temp::<(egui::Pos2, egui::Pos2)>(key) {
                    d.insert_temp(key, (from, p));
                }
            });
        }
        return None;
    }
    let (from, last) = ui.data_mut(|d| d.remove_temp::<(egui::Pos2, egui::Pos2)>(key))?;
    Some(pos.unwrap_or(last) - from)
}
