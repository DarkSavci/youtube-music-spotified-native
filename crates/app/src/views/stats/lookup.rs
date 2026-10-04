//! Looking up any artist, song or album you have played.
//!
//! The search is over your own history, not YouTube's catalogue: it
//! answers "how much have I listened to this", so something never played
//! is simply not found rather than shown with a row of noughts.

use eframe::egui::{self, Frame, Key, Margin, Order, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{LookupResults, StatKind, cover};

use super::plural;
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::views::widgets::{self, ArtShape, TextField};

const WIDTH: f32 = 560.0;
const ITEM_HEIGHT: f32 = 52.0;
const ART: f32 = 40.0;
/// What the field is called, by a screen reader and by a test.
pub const LABEL: &str = "Look up an artist, song or album in your listening";

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let page = &state.stats_page;
    let width = WIDTH.min(ui.available_width());
    let field = TextField {
        text: &page.lookup_text,
        hint: "Look up an artist, song or album you've played",
        label: LABEL,
        icon: Some(Icon::Search),
        width,
        compact: false,
    };
    let top_left = ui.cursor().min;
    if let Some(text) = field.show(ui, &state.palette) {
        actions.push(Action::SetStatsLookup(text));
    }
    let field = Rect::from_min_max(top_left, pos2(top_left.x + width, ui.min_rect().bottom()));
    let asked = page.lookup_text.trim();
    let Some((_, found)) = page
        .lookup
        .as_ref()
        .filter(|(text, _)| page.lookup_open && !asked.is_empty() && text == asked)
    else {
        return;
    };
    let palette = &state.palette;
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke((1.0, palette.outline))
        .corner_radius(theme::RADIUS)
        .shadow(egui::epaint::Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: palette.shadow,
        })
        .inner_margin(Margin::same(8));
    // Over the page, not in it: the matches come and go without moving
    // what is below the field.
    let top = field.bottom() + 8.0;
    // No taller than the window has room for under the field: the rest
    // scrolls.
    let window = ui.ctx().content_rect();
    let most = (window.bottom() - top - 32.0)
        .min(window.height() * 0.6)
        .max(ITEM_HEIGHT * 2.0);
    let area = egui::Area::new(ui.id().with("lookup-results"))
        .order(Order::Foreground)
        .fixed_pos(pos2(field.left(), top))
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            frame.show(ui, |ui| {
                ui.set_width(width - 18.0);
                egui::ScrollArea::vertical()
                    .max_height(most)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        results(state, ui, actions, found, asked);
                    });
            });
        });
    // A click anywhere else, or Escape, puts the matches away.
    let elsewhere = ui.input(|input| {
        let pressed = input.pointer.any_pressed();
        let at = input.pointer.interact_pos();
        pressed && at.is_some_and(|at| !area.response.rect.contains(at) && !field.contains(at))
    });
    if elsewhere || ui.input(|input| input.key_pressed(Key::Escape)) {
        actions.push(Action::CloseStatsLookup);
    }
}

fn results(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    found: &LookupResults,
    asked: &str,
) {
    let palette = &state.palette;
    if found.is_empty() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("Nothing you've played matches “{asked}”."))
                .font(theme::regular(13.0))
                .color(palette.secondary),
        );
        ui.add_space(8.0);
        return;
    }
    let mut pick = |kind: StatKind, id: &str| {
        actions.push(Action::OpenStat {
            kind,
            id: id.to_owned(),
        });
    };
    if !found.artists.is_empty() {
        group_title(state, ui, "ARTISTS");
    }
    for artist in &found.artists {
        let second = format!(
            "{} · {}",
            plural(artist.plays, "play"),
            plural(artist.distinct_tracks, "song")
        );
        let entry = Entry {
            title: &artist.artist,
            second,
            art: &artist.artwork,
            shape: ArtShape::Circle,
        };
        if entry.show(state, ui) {
            pick(StatKind::Artist, &artist.artist_id);
        }
    }
    if !found.tracks.is_empty() {
        group_title(state, ui, "SONGS");
    }
    for track in &found.tracks {
        let entry = Entry {
            title: &track.title,
            second: format!("{} · {}", track.artist, plural(track.plays, "play")),
            art: &track.artwork,
            shape: ArtShape::Rounded(4),
        };
        if entry.show(state, ui) {
            pick(StatKind::Track, &track.track_id);
        }
    }
    if !found.albums.is_empty() {
        group_title(state, ui, "ALBUMS");
    }
    for album in &found.albums {
        let entry = Entry {
            title: &album.album,
            second: format!("{} · {}", album.artist, plural(album.plays, "play")),
            art: &album.artwork,
            shape: ArtShape::Rounded(4),
        };
        if entry.show(state, ui) {
            pick(StatKind::Album, &album.key);
        }
    }
}

fn group_title(state: &State, ui: &mut Ui, title: &str) {
    let palette = &state.palette;
    let font = theme::semibold(11.0);
    let text = widgets::tracked(ui, title, font, palette.dim, 0.7, (f32::MAX, 1));
    let size = vec2(ui.available_width(), 28.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let at = pos2(rect.left() + 12.0, rect.bottom() - 6.0 - text.size().y);
    ui.painter().galley(at, text, palette.dim);
}

/// One match: a cover, a name, and the listener's figures for it.
struct Entry<'a> {
    title: &'a str,
    second: String,
    /// The address of its cover; empty when the play log has none.
    art: &'a str,
    shape: ArtShape,
}

impl Entry<'_> {
    /// Returns whether it was chosen.
    fn show(&self, state: &State, ui: &mut Ui) -> bool {
        let palette = &state.palette;
        let size = vec2(ui.available_width(), ITEM_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        widgets::name(ui, &response, &format!("{}, {}", self.title, self.second));
        widgets::row_hover(ui, &response, rect);
        let art = Rect::from_center_size(
            pos2(rect.left() + 12.0 + ART / 2.0, rect.center().y),
            vec2(ART, ART),
        );
        widgets::artwork(ui, state, &cover(self.art), art, self.shape, Icon::Music);
        let left = art.right() + 12.0;
        let width = (rect.right() - left - 12.0).max(40.0);
        let font = theme::medium(14.0);
        let title = widgets::elided(ui, self.title, font, palette.text, width, 1);
        ui.painter()
            .galley(pos2(left, rect.center().y - 18.0), title, palette.text);
        let font = theme::regular(12.0);
        let second = widgets::elided(ui, &self.second, font, palette.secondary, width, 1);
        ui.painter()
            .galley(pos2(left, rect.center().y + 2.0), second, palette.secondary);
        response.clicked()
    }
}
