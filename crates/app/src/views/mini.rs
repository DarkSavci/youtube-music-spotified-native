//! The mini player: a small window of its own that stays above the others,
//! beside the main window rather than in place of it.
//!
//! What it shows depends only on its size. A strip is a bar: cover, title
//! and three buttons. Roughly square, it is the cover, with the controls
//! over it while the pointer is there. Wide, the cover sits beside the
//! controls. Tall, there is room for the queue or the lyrics as well.

use eframe::egui::{
    self, Align2, Color32, Frame, Rect, Ui, ViewportBuilder, ViewportId, pos2, vec2,
};

use super::widgets;
use super::{chrome, lyrics, queue};
use crate::actions::Action;
use crate::state::{MiniPanel, Playback, State};
use crate::theme;

const MIN_SIZE: [f32; 2] = [360.0, 80.0];
/// What the window grows to, at least, to show the queue or the lyrics.
pub const PANEL_SIZE: [f32; 2] = [360.0, 580.0];

mod parts;

use parts::{
    Mini, cover, drag, extras, like, meta, progress, speed_button, thin_progress, transport, wash,
    window_buttons,
};

const HEAD: f32 = 40.0;
const PADDING: f32 = 12.0;
const THUMB: f32 = 40.0;
/// The cover is asked for at one size whatever the layout, so every
/// layout shares one download and one colour.
const COVER: f32 = 360.0;

pub fn viewport() -> ViewportId {
    ViewportId::from_hash_of("mini-player")
}

/// The window, as it is asked for on every frame. Size and place are the
/// ones it was opened with: asking again for the same changes nothing, so
/// the person's own moving and resizing stand.
pub fn builder(state: &State) -> ViewportBuilder {
    let level = if state.settings.mini_on_top {
        egui::WindowLevel::AlwaysOnTop
    } else {
        egui::WindowLevel::Normal
    };
    let builder = ViewportBuilder::default()
        .with_title("Mini player")
        // Said outright: the window is made hidden, and only a change
        // that is spelt out is passed on to it.
        .with_visible(true)
        .with_decorations(false)
        .with_resizable(true)
        .with_maximize_button(false)
        .with_window_level(level)
        .with_inner_size(state.mini_opened.size)
        .with_min_inner_size(MIN_SIZE);
    match state.mini_opened.position {
        Some(position) => builder.with_position(position),
        None => builder,
    }
}

/// Which arrangement fits a window of this size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    Bar,
    Square,
    Wide,
    Tall,
}

fn layout_for(width: f32, height: f32) -> Layout {
    const SQUARE_MIN_HEIGHT: f32 = 260.0;
    const WIDE_MIN_SIDE: f32 = 270.0;
    if height < 140.0 {
        return Layout::Bar;
    }
    if height >= 400.0 || (height >= 300.0 && width / height < 0.8) {
        return Layout::Tall;
    }
    let square = height >= SQUARE_MIN_HEIGHT;
    let wide = width - height >= WIDE_MIN_SIDE;
    // Nearer square than wide, the cover is tried first; otherwise the
    // side-by-side arrangement is.
    let prefers_square = width / height <= 1.35;
    match (square, wide) {
        (true, true) if prefers_square => Layout::Square,
        (true, true) | (false, true) => Layout::Wide,
        (true, false) => Layout::Square,
        (false, false) => Layout::Bar,
    }
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let playback = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some());
    let mini = Mini {
        state,
        playback,
        track: playback.and_then(Playback::current),
    };
    egui::CentralPanel::default()
        .frame(Frame::new().fill(state.palette.panel))
        .show(ui, |ui| {
            let area = ui.max_rect();
            let art = mini
                .track
                .and_then(|track| widgets::artwork_tint(ui, state, &track.artwork, COVER));
            match layout_for(area.width(), area.height()) {
                Layout::Bar => bar(&mini, ui, actions, area, art),
                Layout::Square => square(&mini, ui, actions, area),
                Layout::Wide => wide(&mini, ui, actions, area, art),
                Layout::Tall => tall(&mini, ui, actions, area, art),
            }
            chrome::resize_edges(ui);
        });
    let (closing, place) = ui.input(|input| {
        let viewport = input.viewport();
        let position = viewport.outer_rect.map(|rect| rect.min);
        let size = viewport.inner_rect.map(|rect| rect.size());
        (viewport.close_requested(), position.zip(size))
    });
    if closing {
        actions.push(Action::ToggleMiniPlayer);
    }
    // Where the window is and how big is remembered for next time.
    if let Some((position, size)) = place {
        let (position, size) = (<[f32; 2]>::from(position), <[f32; 2]>::from(size));
        if state.settings.mini_position != Some(position) || state.settings.mini_size != size {
            actions.push(Action::MiniMoved { position, size });
        }
    }
}

/// A strip: everything on one line, and the whole of it moves the window.
fn bar(mini: &Mini<'_>, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect, art: Option<Color32>) {
    wash(ui, area, art, true);
    drag(ui, area);
    let y = area.center().y;
    let side = (area.height() - 24.0).min(56.0);
    let thumb = Rect::from_min_size(
        pos2(area.left() + PADDING, y - side / 2.0),
        vec2(side, side),
    );
    cover(mini, ui, actions, thumb, 4);
    let right = pos2(area.right() - 8.0, y);
    let mut buttons = window_buttons(mini, ui, actions, right, area.width() > 400.0);
    // The buttons step in as the width allows, the least needed last.
    if area.width() > 500.0 {
        buttons += speed_button(mini, ui, actions, pos2(right.x - buttons, y));
    }
    let transport_centre = pos2(area.right() - 8.0 - buttons - 70.0, y);
    transport(mini, ui, actions, transport_centre, false);
    let mut text_right = transport_centre.x - 66.0;
    if area.width() > 440.0 {
        like(mini, ui, actions, pos2(text_right - 14.0, y));
        text_right -= 34.0;
    }
    let text = Rect::from_min_max(
        pos2(thumb.right() + PADDING, area.top()),
        pos2(text_right, area.bottom()),
    );
    meta(mini, ui, actions, text, false);
    thin_progress(mini, ui, area);
}

/// The cover, edge to edge. The controls lie over it while the pointer is
/// in the window, on a shade that lets them be read on any cover.
fn square(mini: &Mini<'_>, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect) {
    drag(ui, area);
    cover(mini, ui, actions, area, 0);
    let inside = ui.rect_contains_pointer(area);
    let lift = widgets::hover_of(ui, ui.id().with("mini-overlay"), inside);
    if lift <= 0.0 {
        thin_progress(mini, ui, area);
        return;
    }
    let shade = |alpha: f32| Color32::from_black_alpha((alpha * lift * 255.0) as u8);
    let top = area.with_max_y(area.top() + area.height() * 0.28);
    widgets::vertical_gradient(ui, top, shade(0.6), Color32::TRANSPARENT);
    let bottom = area.with_min_y(area.top() + area.height() * 0.36);
    widgets::vertical_gradient(ui, bottom, Color32::TRANSPARENT, shade(0.88));
    // The controls arrive with the pointer, not ahead of it.
    if !inside {
        return;
    }
    let buttons_at = pos2(area.right() - 6.0, area.top() + HEAD / 2.0);
    let buttons = window_buttons(mini, ui, actions, buttons_at, true);
    let speed_at = pos2(buttons_at.x - buttons, buttons_at.y);
    speed_button(mini, ui, actions, speed_at);
    // Built up from the bottom edge; what there is no room for is left out.
    let inner = area.shrink(PADDING);
    let mut y = inner.bottom();
    let mut row = |height: f32| {
        y -= height;
        Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + height))
    };
    let roomy = area.height() > 270.0;
    if roomy {
        extras(mini, ui, actions, row(32.0), true);
    }
    transport(mini, ui, actions, row(44.0).center(), true);
    if area.height() > 210.0 {
        progress(mini, ui, actions, row(20.0), roomy);
    }
    let text = row(40.0);
    like(
        mini,
        ui,
        actions,
        pos2(text.right() - 14.0, text.center().y),
    );
    meta(
        mini,
        ui,
        actions,
        text.with_max_x(text.right() - 36.0),
        true,
    );
}

/// The cover at the left, as tall as the window, and the controls beside.
fn wide(mini: &Mini<'_>, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect, art: Option<Color32>) {
    wash(ui, area, art, false);
    let picture = Rect::from_min_size(area.min, vec2(area.height(), area.height()));
    let side = area.with_min_x(picture.right());
    let head = side.with_max_y(side.top() + HEAD + 8.0);
    drag(ui, head);
    cover(mini, ui, actions, picture, 0);
    let buttons_at = pos2(side.right() - 6.0, side.top() + HEAD / 2.0 + 2.0);
    let mut buttons = window_buttons(mini, ui, actions, buttons_at, true);
    let speed_at = pos2(buttons_at.x - buttons, buttons_at.y);
    buttons += speed_button(mini, ui, actions, speed_at);
    let inner = side.shrink2(vec2(16.0, PADDING));
    let text = Rect::from_min_max(
        pos2(inner.left(), head.top() + 4.0),
        pos2(side.right() - buttons - 44.0, head.bottom()),
    );
    like(
        mini,
        ui,
        actions,
        pos2(text.right() + 16.0, text.center().y),
    );
    meta(mini, ui, actions, text, false);
    // The three rows share what is left under the head evenly.
    let rest = inner.with_min_y(head.bottom());
    let step = rest.height() / 3.0;
    let row = |index: f32, height: f32| {
        let centre = pos2(rest.center().x, rest.top() + step * (index + 0.5));
        Rect::from_center_size(centre, vec2(rest.width(), height))
    };
    progress(mini, ui, actions, row(0.0, 20.0), true);
    transport(mini, ui, actions, row(1.0, 44.0).center(), true);
    extras(mini, ui, actions, row(2.0, 32.0), area.width() > 520.0);
}

/// Room for more: the cover, the queue or the lyrics above the controls.
fn tall(mini: &Mini<'_>, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect, art: Option<Color32>) {
    let palette = &mini.state.palette;
    let panel = mini.state.mini_panel;
    // Lyrics sit on the cover's colour itself; the rest on a wash of it.
    match (panel, art) {
        (MiniPanel::Lyrics, Some(art)) => {
            ui.painter().rect_filled(area, 0.0, art);
        }
        _ => wash(ui, area, art, false),
    }
    let head = area.with_max_y(area.top() + HEAD);
    drag(ui, head);
    let label = match panel {
        MiniPanel::Art => "",
        MiniPanel::Queue => "Queue",
        MiniPanel::Lyrics => "Lyrics",
    };
    let at = pos2(head.left() + 16.0, head.center().y);
    let font = theme::bold(12.5);
    widgets::text_at(ui, at, Align2::LEFT_CENTER, label, font, palette.text);
    let buttons_at = pos2(head.right() - 6.0, head.center().y);
    let buttons = window_buttons(mini, ui, actions, buttons_at, true);
    let speed_at = pos2(buttons_at.x - buttons, buttons_at.y);
    speed_button(mini, ui, actions, speed_at);

    let foot_height = PADDING * 2.0 + 44.0 + 20.0 + 44.0 + 32.0;
    let foot = area.with_min_y(area.bottom() - foot_height);
    let middle = Rect::from_min_max(head.left_bottom(), foot.right_top());
    match panel {
        MiniPanel::Art => {
            let room = middle.shrink2(vec2(24.0, 8.0));
            let side = room.width().min(room.height());
            let picture = Rect::from_center_size(room.center(), vec2(side, side));
            cover(mini, ui, actions, picture, theme::RADIUS);
        }
        MiniPanel::Queue | MiniPanel::Lyrics => {
            let inside = middle.shrink2(vec2(8.0, 0.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inside));
            child.set_clip_rect(inside);
            match (panel, mini.playback) {
                (MiniPanel::Queue, Some(playback)) => {
                    queue::contents(mini.state, &mut child, actions, playback);
                }
                (MiniPanel::Queue, None) => {}
                _ => lyrics::compact(mini.state, &mut child, actions),
            }
        }
    }

    let inner = foot.shrink2(vec2(16.0, PADDING));
    let mut y = inner.top();
    let mut row = |height: f32| {
        let rect = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + height));
        y += height;
        rect
    };
    let title = row(44.0);
    let mut text_left = title.left();
    // With the cover given over to the queue or the lyrics, a small one
    // here still shows what is playing.
    if panel != MiniPanel::Art {
        let at = pos2(title.left(), title.center().y - THUMB / 2.0);
        let thumb = Rect::from_min_size(at, vec2(THUMB, THUMB));
        cover(mini, ui, actions, thumb, 4);
        text_left = thumb.right() + PADDING;
    }
    like(
        mini,
        ui,
        actions,
        pos2(title.right() - 14.0, title.center().y),
    );
    let text = Rect::from_min_max(
        pos2(text_left, title.top()),
        pos2(title.right() - 36.0, title.bottom()),
    );
    meta(mini, ui, actions, text, false);
    progress(mini, ui, actions, row(20.0), true);
    transport(mini, ui, actions, row(44.0).center(), true);
    extras(mini, ui, actions, row(32.0), true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strip_is_a_bar_and_the_default_is_the_cover() {
        assert_eq!(layout_for(360.0, 80.0), Layout::Bar);
        assert_eq!(layout_for(900.0, 139.0), Layout::Bar);
        assert_eq!(layout_for(360.0, 360.0), Layout::Square);
    }

    #[test]
    fn a_window_much_wider_than_tall_puts_the_cover_beside_the_controls() {
        assert_eq!(layout_for(640.0, 280.0), Layout::Wide);
        assert_eq!(layout_for(560.0, 200.0), Layout::Wide);
        // Too short for the cover alone and too narrow to sit beside it.
        assert_eq!(layout_for(400.0, 200.0), Layout::Bar);
    }

    #[test]
    fn a_tall_window_has_room_for_the_queue() {
        assert_eq!(layout_for(360.0, 580.0), Layout::Tall);
        assert_eq!(layout_for(360.0, 400.0), Layout::Tall);
        // Narrow and fairly tall counts too.
        assert_eq!(layout_for(230.0, 300.0), Layout::Tall);
    }
}
