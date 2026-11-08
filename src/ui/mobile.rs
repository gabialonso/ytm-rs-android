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
const MINI_H: f32 = 64.0;
const TABS_H: f32 = 62.0;
/// Now playing's seek bar, buttons and their bottom row.
const CONTROLS_H: f32 = 196.0;

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
    let items: [(char, &str, Page, bool); 3] = [
        (icon::HOUSE, "Home", Page::Home, app.page == Page::Home),
        (icon::COMPASS, "Explore", Page::Explore, app.page == Page::Explore),
        (icon::LIBRARY, "Library", Page::Library(LibTab::Playlists), matches!(app.page, Page::Library(_))),
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

    // Like on the left; Lyrics and Up next (the side panel's two views) on the right.
    let by = r.top() + 152.0;
    if track.is_some() {
        let liked = app.heard_liked();
        let heard_id = app.playing_track().map(|t| t.id.clone()).unwrap_or_default();
        let lr = Rect::from_center_size(pos2(r.left() + 18.0, by), Vec2::splat(44.0));
        let lresp = named(ui.interact(lr, ui.scope_id().with("np-like"), Sense::click()), if liked { "Remove like" } else { "Like" });
        let pop = theme::anim_bool(ui.ctx(), ("liked", &heard_id), liked, 0.3);
        let size = 22.0 * (1.0 + 0.25 * (pop * std::f32::consts::PI).sin());
        widgets::heart(ui, lr.center(), size, pop, theme::lerp_color(dim, theme::RED, pop), theme::RED);
        if lresp.clicked() {
            app.actions.push(Action::LikeHeard(!liked));
        }
    }
    let no_lyrics = app.no_lyrics();
    let mut x = r.right();
    for (tab, glyph, label) in [(NpTab::UpNext, icon::LIST_MUSIC, "Up next"), (NpTab::Lyrics, icon::MIC_VOCAL, "Lyrics")] {
        let on = app.np_tab == tab && !(tab == NpTab::Lyrics && no_lyrics);
        let g = widgets::galley(ui, label, theme::semibold(13.5), Color32::PLACEHOLDER, 200.0);
        let w = g.size().x + 44.0;
        let chip = Rect::from_min_max(pos2(x - w, by - 17.0), pos2(x, by + 17.0));
        x = chip.left() - 8.0;
        let resp = named(ui.interact(chip, ui.scope_id().with(("np-tab", label)), Sense::click()), label);
        let s = theme::anim_bool(ui.ctx(), resp.id.with("s"), on, 0.2);
        let off = tab == NpTab::Lyrics && no_lyrics;
        let a = if off { 0.4 } else { 1.0 };
        ui.painter().rect_filled(chip, 17.0, theme::with_alpha(theme::lerp_color(Color32::from_white_alpha(26), Color32::WHITE, s), a));
        let fg = theme::with_alpha(theme::lerp_color(Color32::WHITE, Color32::BLACK, s), a);
        icon_at(ui, pos2(chip.left() + 20.0, by), glyph, 15.0, fg);
        ui.painter().galley(pos2(chip.left() + 34.0, by - g.size().y / 2.0), g, fg);
        if resp.clicked() && !off {
            // Tapping the view that's showing goes back to the artwork alone.
            app.np_tab = if app.np_tab == tab { NpTab::Art } else { tab };
        }
    }
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

/// A carousel's touch gesture: which way it went, decided once the finger has moved a
/// little. Up and down is the page's, though it started on the carousel.
#[derive(Clone, Copy)]
enum Axis {
    Sideways,
    /// Held at this offset while the page scrolls.
    Page(f32),
    /// The finger let go of a page scroll: it glides on at this speed (points a second).
    Glide(f32),
}

/// The offset to hold a carousel at while a gesture that started on it scrolls the page.
pub(super) fn carousel_hold(ui: &Ui, id: egui::Id) -> Option<f32> {
    match ui.data(|d| d.get_temp::<Axis>(id.with("axis"))) {
        Some(Axis::Page(x)) => Some(x),
        _ => None,
    }
}

/// After a carousel is shown (`rect`, scrolled to `offset`): an up-and-down swipe that
/// started on it goes to the page around it, which egui's nested scrolling wouldn't do.
pub(super) fn carousel_touch(ui: &Ui, id: egui::Id, rect: Rect, offset: f32) {
    if !phone(ui.ctx()) {
        return;
    }
    let key = id.with("axis");
    let state = ui.data(|d| d.get_temp::<Axis>(key));
    let (origin, latest, delta, down, velocity, dt) =
        ui.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos(), i.pointer.delta(), i.pointer.primary_down(), i.pointer.velocity(), i.stable_dt));
    let scroll = |dy: f32| ui.ctx().input_mut(|i| i.smooth_scroll_delta.y += dy);
    match state {
        _ if down && origin.is_some_and(|o| rect.contains(o)) => match state {
            None => {
                if let (Some(o), Some(p)) = (origin, latest) {
                    let moved = p - o;
                    if moved.length() > 10.0 {
                        let axis = if moved.y.abs() > moved.x.abs() { Axis::Page(offset) } else { Axis::Sideways };
                        if let Axis::Page(_) = axis {
                            scroll(moved.y);
                        }
                        ui.data_mut(|d| d.insert_temp(key, axis));
                    }
                }
            }
            Some(Axis::Page(_)) => scroll(delta.y),
            _ => {}
        },
        Some(Axis::Page(_)) => {
            // Let go: the page glides on, as it does when the swipe starts on it.
            ui.data_mut(|d| d.insert_temp(key, Axis::Glide(velocity.y)));
            ui.ctx().request_repaint();
        }
        Some(Axis::Glide(v)) if v.abs() > 20.0 && !down => {
            scroll(v * dt);
            ui.data_mut(|d| d.insert_temp(key, Axis::Glide(v * (-4.0 * dt).exp())));
            ui.ctx().request_repaint();
        }
        Some(_) => ui.data_mut(|d| d.remove::<Axis>(key)),
        None => {}
    }
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
