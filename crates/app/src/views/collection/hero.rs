//! The top of an album's, a playlist's or an artist's page: the cover's
//! colour fading into the page, the cover, what it is, its name and a
//! byline; and the row of buttons under it, which stays in sight when the
//! rest scrolls away.

use eframe::egui::{self, Align2, Color32, Rect, Sense, Ui, Vec2, pos2, vec2};
use spotified_client::models::{Artwork, Track};

use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon};
use crate::views::widgets::{self, ArtShape};
use crate::views::{actions_menu, page_card};

pub const COVER: f32 = 208.0;
pub const COVER_NARROW: f32 = 160.0;
/// At this page width and below, the cover and the room above it shrink.
const NARROW_PAGE: f32 = 720.0;
/// The room above the cover, and below it.
const ABOVE: f32 = 48.0;
const ABOVE_NARROW: f32 = 24.0;
const BELOW: f32 = 24.0;
/// How much of the cover's colour the top of the page takes.
const TINT_STRENGTH: f32 = 0.85;
const TINT_STRENGTH_LIGHT: f32 = 0.22;
/// How much of it the row of buttons takes once it has stuck to the top.
const STUCK_TINT: f32 = 0.45;
const PLAY_BUTTON: f32 = 56.0;
/// The row of buttons, with the room above and below them.
pub const ACTIONS_HEIGHT: f32 = 80.0;
const AVATAR: f32 = 24.0;

/// One part of a byline. Parts are set apart by a dot.
pub struct Part {
    pub text: String,
    /// Set in the text colour and a heavier face: who it is by.
    pub strong: bool,
    /// Where a click on it leads.
    pub page: Option<Page>,
    /// Goes on from the part before it after a space, with no dot
    /// between: the two are one phrase, part of it set heavier.
    pub joined: bool,
}

impl Part {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            strong: false,
            page: None,
            joined: false,
        }
    }

    pub fn strong(text: impl Into<String>, page: Option<Page>) -> Self {
        Self {
            text: text.into(),
            strong: true,
            page,
            joined: false,
        }
    }

    /// The same, going on from the part before it.
    pub fn joined(mut self) -> Self {
        self.joined = true;
        self
    }
}

pub struct Hero<'a> {
    pub art: &'a [Artwork],
    pub shape: ArtShape,
    pub placeholder: Icon,
    pub kind: &'a str,
    pub title: &'a str,
    /// A round picture before the byline: an album's artist.
    pub avatar: &'a [Artwork],
    pub byline: Vec<Part>,
    /// The cover's colour, once it is known.
    pub tint: Option<Color32>,
}

/// The size a title of this length is set at in a window `window` wide,
/// and how much tighter than usual its letters sit, as a part of that
/// size. A hero size suits a few words; a long title set that large would
/// run to six lines, so longer ones step down.
fn title_size(title: &str, window: f32) -> (f32, f32) {
    match title.chars().count() {
        ..=24 => ((window * 0.06).clamp(32.0, 72.0), -0.04),
        25..=45 => ((window * 0.045).clamp(32.0, 56.0), -0.04),
        46..=80 => ((window * 0.032).clamp(24.0, 48.0), -0.03),
        _ => ((window * 0.024).clamp(20.0, 36.0), -0.02),
    }
}

/// The colour the top of the page takes from the cover.
fn tinted(state: &State, tint: Color32, strength: f32) -> Color32 {
    let palette = &state.palette;
    // The tint is a dark colour: a light page takes a breath of it, where
    // a dark one takes it nearly whole.
    let most = if palette.dark {
        TINT_STRENGTH
    } else {
        TINT_STRENGTH_LIGHT
    };
    crate::tint::blend(palette.panel, tint, most * strength)
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, hero: Hero<'_>) {
    let palette = &state.palette;
    let narrow = ui.available_width() <= NARROW_PAGE;
    let (cover, above) = if narrow {
        (COVER_NARROW, ABOVE_NARROW)
    } else {
        (COVER, ABOVE)
    };
    let size = vec2(ui.available_width(), above + cover + BELOW);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    // The colour runs from edge to edge of the card, behind everything,
    // and has gone by the bottom of the header.
    if let Some(tint) = hero.tint {
        let card = page_card(ui);
        let across = Rect::from_x_y_ranges(card.x_range(), rect.y_range());
        let (upper, lower) = across.split_top_bottom_at_fraction(0.7);
        let (top, middle) = (tinted(state, tint, 1.0), tinted(state, tint, 0.4));
        widgets::vertical_gradient(ui, upper, top, middle);
        widgets::vertical_gradient(ui, lower, middle, palette.panel);
    }
    let image = Rect::from_min_size(rect.min + vec2(0.0, above), vec2(cover, cover));
    // The cover stands a little off the page.
    let shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    let radius = match hero.shape {
        ArtShape::Rounded(radius) => egui::CornerRadius::same(radius),
        ArtShape::Circle => egui::CornerRadius::same(u8::MAX),
    };
    ui.painter().add(shadow.as_shape(image, radius));
    widgets::artwork(ui, state, hero.art, image, hero.shape, hero.placeholder);

    let left = image.right() + 24.0;
    let width = (rect.right() - left).max(80.0);
    // Stacked upwards from the cover's bottom edge: byline, title, kind.
    let byline_height = if hero.avatar.is_empty() { 18.0 } else { AVATAR };
    let byline_middle = image.bottom() - byline_height / 2.0;
    byline(state, ui, actions, &hero, pos2(left, byline_middle), width);

    let window = ui.ctx().content_rect().width();
    let (size, tracking) = title_size(hero.title, window);
    let font = theme::bold(size);
    let wrap = (width, 3);
    let title = widgets::tracked(ui, hero.title, font, palette.text, size * tracking, wrap);
    let title_top = image.bottom() - byline_height - 12.0 - title.size().y;
    // Set a touch to the left, so a large first letter lines up with the
    // small text above and below it.
    ui.painter()
        .galley(pos2(left - size * 0.04, title_top), title, palette.text);
    let font = theme::semibold(12.0);
    let kind = hero.kind.to_uppercase();
    let kind = widgets::tracked(ui, &kind, font, palette.text, 1.0, (width, 1));
    let at = pos2(left, title_top - 12.0 - kind.size().y);
    ui.painter().galley(at, kind, palette.text);
}

/// The line under the title: who it is by, then what there is of it, with
/// a dot between neighbours.
fn byline(
    state: &State,
    ui: &Ui,
    actions: &mut Vec<Action>,
    hero: &Hero<'_>,
    at: egui::Pos2,
    width: f32,
) {
    let palette = &state.palette;
    let mut x = at.x;
    if !hero.avatar.is_empty() {
        let disc = Rect::from_min_size(pos2(x, at.y - AVATAR / 2.0), Vec2::splat(AVATAR));
        widgets::artwork(ui, state, hero.avatar, disc, ArtShape::Circle, Icon::User);
        x += AVATAR + 8.0;
    }
    let right = at.x + width;
    let shown = hero.byline.iter().filter(|part| !part.text.is_empty());
    for (index, part) in shown.enumerate() {
        if index > 0 && part.joined {
            x += 4.0;
        } else if index > 0 {
            let font = theme::regular(12.0);
            let dot = pos2(x + 8.0, at.y);
            widgets::text_at(ui, dot, Align2::CENTER_CENTER, "·", font, palette.secondary);
            x += 16.0;
        }
        let (font, color) = if part.strong {
            (theme::semibold(12.0), palette.text)
        } else {
            (theme::regular(12.0), palette.secondary)
        };
        let link = widgets::Link {
            text: &part.text,
            font,
            color,
            width: (right - x).max(20.0),
        };
        let id = ui.id().with(("byline", index));
        let (clicked, drawn) = link.show_measured(ui, id, pos2(x, at.y - 8.0), part.page.is_some());
        if let (true, Some(page)) = (clicked, &part.page) {
            actions.push(Action::Open(page.clone()));
        }
        x += drawn;
    }
}

/// What the big button under a hero plays.
#[derive(Clone, Copy)]
pub enum Plays<'a> {
    /// These, from the first that can be played.
    Tracks(&'a [Track]),
    /// A playlist: all of it, though only these of its songs may be here.
    Playlist(&'a str, &'a [Track]),
    /// Every song of an artist's, most played first. `any` is whether
    /// there is anything of theirs to play.
    Artist { id: &'a str, any: bool },
}

/// The big button under a hero: plays the collection from the top, or
/// pauses it when it is what is playing.
pub fn play_button(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    plays: Plays<'_>,
    origin: &str,
) {
    let first = match plays {
        Plays::Tracks(tracks) | Plays::Playlist(_, tracks) => {
            tracks.iter().position(|track| track.playable)
        }
        Plays::Artist { any, .. } => any.then_some(0),
    };
    let palette = &state.palette;
    let playing_this = state.playback.as_ref().is_some_and(|playback| {
        playback.wants_to_play() && playback.session.queue.origin == origin
    });
    let sense = if first.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(PLAY_BUTTON), sense);
    let label = if playing_this {
        "Pause".to_owned()
    } else {
        format!("Play {origin}")
    };
    widgets::name(ui, &response, &label);
    let lift = widgets::hover(ui, &response);
    let fill = match first {
        Some(_) => crate::tint::blend(palette.accent, palette.accent_hover, lift),
        None => palette.surface_active,
    };
    // It swells a little under the pointer and gives when pressed.
    let pressed = if response.is_pointer_button_down_on() {
        0.96
    } else {
        1.0 + 0.06 * lift
    };
    let centre = rect.center();
    let radius = PLAY_BUTTON / 2.0 * pressed;
    ui.painter().circle_filled(
        centre + vec2(0.0, 4.0),
        radius,
        palette.shadow.gamma_multiply(0.5),
    );
    ui.painter().circle_filled(centre, radius, fill);
    let icon = if playing_this {
        Icon::PauseFilled
    } else {
        Icon::PlayFilled
    };
    widgets::paint_icon(ui, icon, rect, 22.0, palette.on_accent);
    let tooltip = if playing_this { "Pause" } else { "Play" };
    if let (true, Some(first)) = (response.on_hover_text(tooltip).clicked(), first) {
        actions.push(match plays {
            _ if playing_this => Action::TogglePlay,
            Plays::Tracks(tracks) => Action::Play {
                tracks: tracks.to_vec(),
                index: first,
                origin: origin.to_owned(),
            },
            Plays::Playlist(id, _) => Action::PlayPlaylist {
                id: id.to_owned(),
                index: first,
            },
            Plays::Artist { id, .. } => Action::PlayArtist {
                artist_id: id.to_owned(),
                shuffle: false,
            },
        });
    }
}

/// A round button that is only an icon, `size` across, with a name of its
/// own apart from its tooltip.
pub fn round_button(
    state: &State,
    ui: &mut Ui,
    (icon, size, glyph): (Icon, f32, f32),
    (name, tooltip): (&str, &str),
) -> egui::Response {
    let palette = &state.palette;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    widgets::name(ui, &response, name);
    let lift = widgets::hover(ui, &response);
    ui.painter()
        .circle_filled(rect.center(), size / 2.0, widgets::wash(ui, lift));
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let pressed = if response.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    widgets::paint_icon(ui, icon, rect, glyph * pressed, color);
    response.on_hover_text(tooltip)
}

/// The "…" button, with the menu of what can be done with the whole page.
pub fn more_button(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    entity: &actions_menu::Entity,
) {
    let name = format!("More options for {}", entity.title);
    let glyph = (Icon::Ellipsis, 32.0, 22.0);
    let button = round_button(state, ui, glyph, (&name, "More options"));
    actions_menu::show(state, actions, &button, entity);
}

/// The row of buttons under an album's or a playlist's hero: Play and "…".
/// Returns the place it should be drawn again, pinned to the top of the
/// page, when its own place has scrolled out of sight; it has then drawn
/// nothing, and [`stuck`] is to be called once the rest of the page is.
pub fn actions(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    entity: &actions_menu::Entity,
) -> Option<Rect> {
    let size = vec2(ui.available_width(), ACTIONS_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let card = page_card(ui);
    if rect.top() < card.top() {
        return Some(Rect::from_min_size(
            card.min,
            vec2(card.width(), ACTIONS_HEIGHT),
        ));
    }
    buttons(state, ui, actions, entity, rect, false);
    None
}

/// The row of buttons pinned to the top of the page at `bar`, over what
/// has scrolled under it, with the page's name beside Play.
pub fn stuck(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    entity: &actions_menu::Entity,
    (bar, tint): (Rect, Option<Color32>),
) {
    let palette = &state.palette;
    let fill = match tint {
        Some(tint) => tinted(state, tint, STUCK_TINT / TINT_STRENGTH),
        None => palette.panel,
    };
    // What is under the bar is neither seen nor clicked through it.
    ui.interact(bar, ui.id().with("stuck-actions"), Sense::click());
    ui.painter().rect_filled(bar, 0.0, fill);
    let inner = bar.shrink2(vec2(theme::PAGE_PADDING, 0.0));
    buttons(state, ui, actions, entity, inner, true);
}

fn buttons(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    entity: &actions_menu::Entity,
    rect: Rect,
    with_title: bool,
) {
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing.x = 16.0;
    // A playlist is played whole, however much of it has been read.
    let plays = if entity.kind == crate::share::Kind::Playlist {
        Plays::Playlist(entity.id, entity.tracks)
    } else {
        Plays::Tracks(entity.tracks)
    };
    play_button(state, &mut row, actions, plays, entity.title);
    if with_title {
        // The name takes what Play and the menu button leave it.
        let room = (row.available_width() - 32.0 - 16.0).max(40.0);
        let font = theme::bold(24.0);
        let color = state.palette.text;
        let title = widgets::tracked(&row, entity.title, font, color, -0.48, (room, 1));
        let (place, _) = row.allocate_exact_size(title.size(), Sense::hover());
        row.painter().galley(place.min, title, color);
    }
    more_button(state, &mut row, actions, entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_title_is_set_smaller_than_a_short_one() {
        let sizes = [
            title_size("Discovery", 1240.0).0,
            title_size("The Rise and Fall of Ziggy Stardust", 1240.0).0,
            title_size(&"word ".repeat(12), 1240.0).0,
            title_size(&"word ".repeat(20), 1240.0).0,
        ];
        assert_eq!(sizes[0], 72.0);
        assert!(sizes.windows(2).all(|pair| pair[0] > pair[1]), "{sizes:?}");
    }

    #[test]
    fn a_title_shrinks_with_the_window_but_not_past_its_least() {
        assert_eq!(title_size("Discovery", 800.0).0, 48.0);
        assert_eq!(title_size("Discovery", 400.0).0, 32.0);
    }
}
