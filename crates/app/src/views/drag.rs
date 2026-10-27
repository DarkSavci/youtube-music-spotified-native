//! Songs carried by the pointer.
//!
//! A row, or a whole selection, can be dragged onto a playlist in the
//! sidebar, onto Liked Music, or onto the queue. While it is carried, a
//! card follows the pointer showing what it is, and, over somewhere it can
//! be let go, what letting go there will do. The rows it was picked up
//! from fade while it is away from them.

use eframe::egui::{self, Align2, Color32, CursorIcon, Id, Rect, Response, Ui, pos2, vec2};
use spotified_client::models::{Artwork, Track};

use super::widgets::{self, ArtShape};
use crate::state::State;
use crate::theme::{self, Icon};

/// What a drag carries: the songs, in the order they were shown.
pub struct Dragged {
    pub tracks: Vec<Track>,
    /// The list whose selection is carried, if it is a selection rather
    /// than the one row that was picked up.
    selection: Option<Id>,
}

/// What the card under the pointer shows.
#[derive(Clone, Default)]
pub struct Card {
    pub art: Vec<Artwork>,
    pub title: String,
    pub line: String,
    /// How many things are carried; more than one stacks the card.
    pub count: usize,
}

const COVER: f32 = 36.0;
const PADDING: f32 = 8.0;
const TEXT_WIDTH: f32 = 220.0;
/// How far the rows a drag was picked up from fade while it is away.
const LEFT_BEHIND: f32 = 0.6;

fn card_id() -> Id {
    Id::new("drag-card")
}

fn hint_id() -> Id {
    Id::new("drag-hint")
}

/// Starts carrying `tracks` if `response` has just begun to be dragged.
/// The songs are only gathered then, not on every frame. `selection` is
/// the list whose selected rows are carried, if they are.
pub fn start(response: &Response, selection: Option<Id>, tracks: impl FnOnce() -> Vec<Track>) {
    if response.drag_started() {
        response.dnd_set_drag_payload(Dragged {
            tracks: tracks(),
            selection,
        });
    }
}

/// Whether the row answering to `response`, in `list`, is among the songs
/// being carried: the row itself, or any selected row if the selection is.
pub fn carries(ui: &Ui, response: &Response, list: Id, selected: bool) -> bool {
    egui::DragAndDrop::payload::<Dragged>(ui.ctx())
        .is_some_and(|dragged| response.dragged() || (selected && dragged.selection == Some(list)))
}

/// Fades a row in `rect` that what is carried was picked up from.
pub fn left_behind(state: &State, ui: &Ui, rect: Rect) {
    let veil = state.palette.panel.gamma_multiply(LEFT_BEHIND);
    ui.painter().rect_filled(rect, theme::RADIUS_ROW, veil);
}

/// Marks `response` as somewhere songs can be dropped: lit while they are
/// held over it, with `hint` saying on the card what letting go will do.
/// Returns the songs on the frame they are let go there.
pub fn target(
    state: &State,
    ui: &Ui,
    response: &Response,
    hint: impl FnOnce() -> String,
) -> Option<Vec<Track>> {
    let over = response.dnd_hover_payload::<Dragged>().is_some();
    if over {
        say(ui.ctx(), hint());
    }
    let time = if widgets::still(ui.ctx()) {
        0.0
    } else {
        theme::ANIMATION_TIME
    };
    let lit = ui
        .ctx()
        .animate_bool_with_time(response.id.with("drop"), over, time);
    if lit > 0.0 {
        let accent = state.palette.accent;
        ui.painter().rect(
            response.rect,
            theme::RADIUS_ROW,
            accent.gamma_multiply(0.16 * lit),
            (1.5, accent.gamma_multiply(lit)),
            egui::StrokeKind::Inside,
        );
    }
    response
        .dnd_release_payload::<Dragged>()
        .map(|dragged| dragged.tracks.clone())
}

/// Says on the card, this frame, what letting go where it is will do.
pub fn say(ctx: &egui::Context, hint: String) {
    ctx.data_mut(|data| data.insert_temp(hint_id(), hint));
}

/// Shows `card` under the pointer this frame, for something carried that
/// is not songs: a row of the queue being moved within it.
pub fn show(ctx: &egui::Context, card: Card) {
    ctx.data_mut(|data| data.insert_temp(card_id(), card));
}

/// The card that follows the pointer while something is carried. Drawn
/// last, after everything that can say what it is or where it would land.
pub fn ghost(state: &State, ui: &Ui) {
    let ctx = ui.ctx();
    let (card, hint) = ctx.data_mut(|data| {
        (
            data.remove_temp::<Card>(card_id()),
            data.remove_temp::<String>(hint_id()),
        )
    });
    let card = card.or_else(|| {
        egui::DragAndDrop::payload::<Dragged>(ctx).and_then(|dragged| songs(&dragged.tracks))
    });
    let pointer = ctx.pointer_interact_pos();
    let time = if widgets::still(ctx) {
        0.0
    } else {
        theme::ANIMATION_TIME
    };
    let shown = ctx.animate_bool_with_time(card_id(), card.is_some() && pointer.is_some(), time);
    let (Some(card), Some(pointer)) = (card, pointer) else {
        return;
    };
    ctx.set_cursor_icon(CursorIcon::Grabbing);
    paint(state, ui, &card, hint.as_deref(), pointer, shown);
}

/// The card for songs: the first one's, and how many more come with it.
fn songs(tracks: &[Track]) -> Option<Card> {
    let first = tracks.first()?;
    let line = match tracks.len() {
        1 => first.artist_names(),
        count => format!("{count} songs"),
    };
    Some(Card {
        art: first.artwork.clone(),
        title: first.title.clone(),
        line,
        count: tracks.len(),
    })
}

fn paint(state: &State, ui: &Ui, card: &Card, hint: Option<&str>, pointer: egui::Pos2, shown: f32) {
    let palette = &state.palette;
    let layer = egui::LayerId::new(egui::Order::Tooltip, card_id());
    let mut ghost = Ui::new(
        ui.ctx().clone(),
        card_id(),
        egui::UiBuilder::new().layer_id(layer),
    );
    ghost.multiply_opacity(shown);
    let painter = ghost.painter().clone();

    let title = widgets::elided(
        &ghost,
        &card.title,
        theme::medium(13.0),
        palette.text,
        TEXT_WIDTH,
        1,
    );
    // Over somewhere it can land, the second line says what landing does.
    let (line, line_color) = match hint {
        Some(hint) => (hint, palette.accent),
        None => (card.line.as_str(), palette.secondary),
    };
    let line = widgets::elided(
        &ghost,
        line,
        theme::regular(12.0),
        line_color,
        TEXT_WIDTH,
        1,
    );
    let text_width = title.size().x.max(line.size().x);
    let size = vec2(
        PADDING + COVER + 10.0 + text_width + PADDING + 4.0,
        COVER + PADDING * 2.0,
    );
    // Rising into place as it appears, a little below the pointer's tip.
    // Near the window's right or bottom edge it goes to the other side of
    // the pointer, so it is never cut off.
    let window = ui.ctx().content_rect();
    let mut offset = vec2(14.0, 10.0 + 6.0 * (1.0 - shown));
    if pointer.x + offset.x + size.x + 12.0 > window.right() {
        offset.x = -14.0 - size.x;
    }
    if pointer.y + offset.y + size.y + 8.0 > window.bottom() {
        offset.y = -10.0 - size.y;
    }
    let rect = Align2::LEFT_TOP.anchor_size(pointer + offset, size);
    let radius = theme::RADIUS;

    // Several things carried are a stack: the cards under it peek out.
    let under = card.count.min(3).saturating_sub(1);
    for depth in (1..=under).rev() {
        let offset = 4.0 * depth as f32;
        let behind = rect.translate(vec2(offset, offset));
        painter.rect(
            behind,
            radius,
            palette.overlay,
            (1.0, palette.outline),
            egui::StrokeKind::Inside,
        );
    }
    let shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: palette.shadow,
    };
    painter.add(shadow.as_shape(rect, radius));
    let border = if hint.is_some() {
        palette.accent
    } else {
        palette.outline
    };
    painter.rect(
        rect,
        radius,
        palette.overlay,
        (1.0, border),
        egui::StrokeKind::Inside,
    );

    let cover = Rect::from_min_size(rect.min + vec2(PADDING, PADDING), vec2(COVER, COVER));
    widgets::artwork(
        &ghost,
        state,
        &card.art,
        cover,
        ArtShape::Rounded(4),
        Icon::Music,
    );
    let left = cover.right() + 10.0;
    let middle = rect.center().y;
    painter.galley(pos2(left, middle - 17.0), title, palette.text);
    painter.galley(pos2(left, middle + 1.0), line, line_color);

    if card.count > 1 {
        badge(&painter, palette, rect.right_top(), card.count);
    }
}

/// How many are carried, on a pill astride the card's top right corner.
fn badge(painter: &egui::Painter, palette: &theme::Palette, corner: egui::Pos2, count: usize) {
    let text = painter.layout_no_wrap(count.to_string(), theme::bold(11.0), palette.on_accent);
    let height = 20.0;
    let width = (text.size().x + 12.0).max(height);
    let pill = Rect::from_center_size(corner + vec2(-4.0, 2.0), vec2(width, height));
    painter.rect(
        pill,
        u8::MAX,
        palette.accent,
        (2.0, palette.overlay),
        egui::StrokeKind::Outside,
    );
    let at = pill.center() - text.size() / 2.0;
    painter.galley(at, text, Color32::PLACEHOLDER);
}
