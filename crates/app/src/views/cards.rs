//! Cards and the shelves that hold them.

use eframe::egui::{self, Rect, Response, Sense, Ui, pos2, vec2};
use spotified_client::models::{
    Album, Artist, Artwork, Item, Mix, Playlist, Podcast, Track, join_names,
};

use super::menus;
use super::widgets::menu::{self, Entry, Menu};
use super::widgets::{self, ArtShape};
use crate::actions::Action;
use crate::blocked::Kind;
use crate::state::{Page, State};
use crate::theme::{self, Icon};

/// The narrowest a card in a row gets; a row holds as many as fit at this
/// width, between the least and the most, and they share it evenly.
const ROW_CARD_MIN: f32 = 168.0;
const ROW_CARDS: std::ops::RangeInclusive<usize> = 2..=8;
/// The same for a grid, which wraps instead of stopping.
const GRID_CARD_MIN: f32 = 180.0;
const GAP: f32 = 16.0;
const PADDING: f32 = 12.0;
const TITLE_HEIGHT: f32 = 18.0;
/// Two lines are reserved whether or not the subtitle fills them, so a row
/// of cards keeps one baseline.
const SUBTITLE_HEIGHT: f32 = 34.0;
/// What a card has beside its square cover: the room around it, and its
/// two captions.
const BESIDE_COVER: f32 = PADDING + 12.0 + TITLE_HEIGHT + 2.0 + SUBTITLE_HEIGHT + PADDING;
const PLAY_BUTTON: f32 = 44.0;
/// How far below its place the play button starts as it fades in.
const PLAY_RISE: f32 = 8.0;
/// How far in from the cover's bottom right corner the play button's
/// centre sits.
const PLAY_INSET: f32 = 30.0;

/// How tall a card `width` wide is.
fn height(width: f32) -> f32 {
    width - PADDING * 2.0 + BESIDE_COVER
}

struct Card<'a> {
    art: &'a [Artwork],
    shape: ArtShape,
    placeholder: Icon,
    title: &'a str,
    subtitle: String,
    /// Whether a play button appears under the pointer.
    playable: bool,
}

/// What was done to a card this frame.
struct CardResponse {
    /// The card itself: a click opens what it shows.
    card: Response,
    /// The play button that appears under the pointer was clicked.
    play: bool,
}

/// A card as wide as the room it is given: a row or a grid hands each of
/// its cards a place of the width they share.
fn card(state: &State, ui: &mut Ui, card: Card<'_>) -> CardResponse {
    let palette = &state.palette;
    let width = ui.available_width();
    let side = width - PADDING * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height(width)), Sense::click());
    widgets::name(ui, &response, card.title);
    if !ui.is_rect_visible(rect) {
        return CardResponse {
            card: response,
            play: false,
        };
    }
    let image = Rect::from_min_size(rect.min + vec2(PADDING, PADDING), vec2(side, side));
    // The button sits over the card, so the card's own hover would flicker
    // off as the pointer reached it; the pointer being anywhere inside the
    // card is what counts.
    let button = Rect::from_center_size(
        image.right_bottom() - vec2(PLAY_INSET, PLAY_INSET),
        vec2(PLAY_BUTTON, PLAY_BUTTON),
    );
    let inside = ui.rect_contains_pointer(rect);
    let lift = widgets::hover_of(ui, response.id, inside);
    if lift > 0.0 {
        let fill = palette.surface.gamma_multiply(lift);
        ui.painter().rect_filled(rect, theme::RADIUS, fill);
    }
    widgets::artwork(ui, state, card.art, image, card.shape, card.placeholder);

    let title_top = image.bottom() + 12.0;
    let title = widgets::elided(ui, card.title, theme::semibold(14.0), palette.text, side, 1);
    ui.painter()
        .galley(pos2(image.left(), title_top), title, palette.text);
    let subtitle = widgets::elided(
        ui,
        &card.subtitle,
        theme::regular(12.0),
        palette.secondary,
        side,
        2,
    );
    ui.painter().galley(
        pos2(image.left(), title_top + TITLE_HEIGHT + 2.0),
        subtitle,
        palette.secondary,
    );

    let mut play = false;
    if lift > 0.0 && card.playable {
        // The button fades in and rises into place as the pointer arrives.
        let button = button.translate(vec2(0.0, PLAY_RISE * (1.0 - lift)));
        let over = if inside {
            let pressed = ui.interact(button, response.id.with("play"), Sense::click());
            widgets::name(ui, &pressed, &format!("Play {}", card.title));
            play = pressed.clicked();
            widgets::hover(ui, &pressed)
        } else {
            0.0
        };
        let fill = crate::tint::blend(palette.accent, palette.accent_hover, over);
        let radius = PLAY_BUTTON / 2.0 + over * 1.5;
        let centre = button.center();
        ui.painter().circle_filled(
            centre + vec2(0.0, 3.0),
            radius + 1.0,
            palette.shadow.gamma_multiply(lift),
        );
        ui.painter()
            .circle_filled(centre, radius, fill.gamma_multiply(lift));
        let glyph = palette.on_accent.gamma_multiply(lift);
        widgets::paint_icon(ui, Icon::PlayFilled, button, 18.0, glyph);
    }
    CardResponse {
        card: response,
        play,
    }
}

/// The menu on an album, a playlist or an artist, which is called `title`.
fn collection_menu(
    state: &State,
    menu: &mut Menu<'_>,
    actions: &mut Vec<Action>,
    (page, title): (&Page, &str),
    link: String,
) {
    // What it is called is what the entry that opens it says, as the
    // Electron app's did: "Open album", "Open show".
    let (open, playable) = match page {
        Page::Album(_) => ("Open album", true),
        Page::Artist(_) => ("Open artist", true),
        Page::Podcast(_) => ("Open show", false),
        _ => ("Open playlist", true),
    };
    if playable && menu.entry(Entry::new("Play").named("Play this")) {
        actions.push(Action::PlayCollection(page.clone()));
    }
    let icon = match page {
        Page::Album(_) => Icon::Disc,
        Page::Artist(_) => Icon::User,
        Page::Podcast(_) => Icon::MicVocal,
        _ => Icon::ListMusic,
    };
    if menu.entry(Entry::new(open).icon(icon)) {
        actions.push(Action::Open(page.clone()));
    }
    let blockable = match page {
        Page::Album(id) => Some((Kind::Album, id)),
        Page::Artist(id) => Some((Kind::Artist, id)),
        _ => None,
    };
    if let Some((kind, id)) = blockable {
        menu.separator();
        actions.extend(menus::block(state, menu, kind, id, title));
    }
    menu.separator();
    if menu.item("Share") {
        actions.push(Action::CopyLink(link));
    }
}

/// A card for something with a page of its own: a click opens the page,
/// the play button plays it, and a right click offers both.
fn collection_card(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    card_spec: Card<'_>,
    page: Page,
    link: String,
) {
    let title = card_spec.title;
    let response = card(state, ui, card_spec);
    menu::context(&response.card, &state.palette, |menu| {
        collection_menu(state, menu, actions, (&page, title), link);
    });
    if response.play {
        actions.push(Action::PlayCollection(page));
    } else if response.card.clicked() {
        actions.push(Action::Open(page));
    }
}

pub fn album(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, album: &Album) {
    let kind = if album.kind.is_empty() {
        "Album"
    } else {
        &album.kind
    };
    // Whose it is; failing that when it is from, or what it is.
    let artists = join_names(&album.artists);
    let subtitle = [artists.as_str(), &album.year, kind]
        .into_iter()
        .find(|text| !text.is_empty())
        .unwrap_or_default();
    let spec = Card {
        art: &album.artwork,
        shape: ArtShape::Rounded(4),
        placeholder: Icon::Music,
        title: &album.title,
        subtitle: subtitle.to_owned(),
        playable: true,
    };
    let link = format!("https://music.youtube.com/browse/{}", album.id);
    collection_card(
        state,
        ui,
        actions,
        spec,
        Page::Album(album.id.clone()),
        link,
    );
}

pub fn artist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist: &Artist) {
    let who = (artist.id.as_str(), artist.name.as_str());
    let second = or(&artist.subscribers, "Artist");
    artist_as(state, ui, actions, who, &artist.artwork, second);
}

/// An artist's card from what is known of them, for a caller with less
/// than a whole artist: their channel and name, a picture, and a second
/// line of its choosing.
pub fn artist_as(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (id, name): (&str, &str),
    art: &[Artwork],
    subtitle: String,
) {
    let spec = Card {
        art,
        shape: ArtShape::Circle,
        placeholder: Icon::User,
        title: name,
        subtitle,
        playable: true,
    };
    let link = format!("https://music.youtube.com/channel/{id}");
    collection_card(state, ui, actions, spec, Page::Artist(id.to_owned()), link);
}

pub fn playlist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, playlist: &Playlist) {
    let spec = Card {
        art: &playlist.artwork,
        shape: ArtShape::Rounded(4),
        placeholder: Icon::ListMusic,
        title: &playlist.title,
        subtitle: or(&playlist.description, "Playlist"),
        playable: true,
    };
    let link = format!("https://music.youtube.com/playlist?list={}", playlist.id);
    let page = Page::Playlist(playlist.id.clone());
    collection_card(state, ui, actions, spec, page, link);
}

pub fn mix(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, mix: &Mix) {
    let cover = mix.tracks.first().map_or(&[][..], |track| &track.artwork);
    let spec = Card {
        art: cover,
        shape: ArtShape::Rounded(4),
        placeholder: Icon::ListMusic,
        title: &mix.title,
        subtitle: mix.description.clone(),
        playable: !mix.tracks.is_empty(),
    };
    let response = card(state, ui, spec);
    let page = Page::Mix(mix.id.clone());
    if response.play {
        actions.push(Action::PlayCollection(page));
    } else if response.card.clicked() {
        actions.push(Action::Open(page));
    }
}

pub fn podcast(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, podcast: &Podcast) {
    let spec = Card {
        art: &podcast.artwork,
        shape: ArtShape::Rounded(4),
        placeholder: Icon::MicVocal,
        title: &podcast.title,
        subtitle: or(&podcast.author, "Podcast"),
        playable: true,
    };
    let link = format!("https://music.youtube.com/playlist?list={}", podcast.id);
    let page = Page::Podcast(podcast.id.clone());
    collection_card(state, ui, actions, spec, page, link);
}

/// A song shown as a card. It has no page to open, so the whole card is
/// the play control: a song picked from a shelf starts its radio, as
/// YouTube Music does. An episode is played by itself.
pub fn track(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, track: &Track) {
    song(state, ui, actions, track, true);
}

fn song(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, track: &Track, radio: bool) {
    let response = card(
        state,
        ui,
        Card {
            art: &track.artwork,
            shape: ArtShape::Rounded(4),
            placeholder: Icon::Music,
            title: &track.title,
            // Who it is by, and nothing more.
            subtitle: track.artist_names(),
            playable: track.playable,
        },
    );
    menu::context(&response.card, &state.palette, |menu| {
        menus::tracks(state, menu, actions, &[track], None);
    });
    if (response.play || response.card.clicked()) && track.playable {
        actions.push(if radio {
            Action::StartRadio(track.clone())
        } else {
            Action::Play {
                tracks: vec![track.clone()],
                index: 0,
                origin: track.title.clone(),
            }
        });
    }
}

pub fn item(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, item: &Item) {
    match item {
        Item::Track(value) => track(state, ui, actions, value),
        Item::Album(value) => album(state, ui, actions, value),
        Item::Artist(value) => artist(state, ui, actions, value),
        Item::Playlist(value) => playlist(state, ui, actions, value),
        Item::Podcast(value) => podcast(state, ui, actions, value),
        Item::Episode(value) => song(state, ui, actions, value, false),
    }
}

/// `text`, or `otherwise` when there is none.
fn or(text: &str, otherwise: &str) -> String {
    if text.is_empty() { otherwise } else { text }.to_owned()
}

/// The room above a section's title, and between it and what it heads.
const TITLE_ABOVE: f32 = 32.0;
const TITLE_BELOW: f32 = 12.0;

pub fn section_title(ui: &mut Ui, title: &str) {
    ui.add_space(TITLE_ABOVE);
    title_text(ui, title);
    ui.add_space(TITLE_BELOW);
}

/// A section's title: large, and set a little tight.
fn title_text(ui: &mut Ui, title: &str) {
    let color = ui.visuals().text_color();
    let room = (ui.available_width(), 1);
    let galley = widgets::tracked(ui, title, theme::bold(24.0), color, -0.48, room);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

/// A section's title, with a way to the whole of it when the section
/// shows only a part. `link` is what that way is called.
pub fn linked_title(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (title, link): (&str, &str),
    whole: Option<Page>,
) {
    let link = whole.is_some().then_some(link);
    if let (true, Some(whole)) = (title_with(state, ui, title, link), whole) {
        actions.push(Action::Open(whole));
    }
}

/// A section's title, with something to click at the right of its line
/// when `link` names it. Returns whether that was clicked.
pub fn title_with(state: &State, ui: &mut Ui, title: &str, link: Option<&str>) -> bool {
    let palette = &state.palette;
    ui.add_space(TITLE_ABOVE);
    let line = ui.available_rect_before_wrap();
    title_text(ui, title);
    let mut clicked = false;
    if let Some(link) = link {
        // At the right, sitting on the title's line.
        let font = theme::bold(12.0);
        let text = widgets::tracked(ui, link, font, palette.secondary, 0.5, (f32::MAX, 1));
        let size = text.size();
        let bottom = ui.min_rect().bottom() - 5.0;
        let rect = Rect::from_min_size(pos2(line.right() - size.x, bottom - size.y), size);
        let response = ui.interact(rect, ui.id().with(("whole", title)), Sense::click());
        widgets::name(ui, &response, link);
        let lift = widgets::hover(ui, &response);
        let color = crate::tint::blend(palette.secondary, palette.text, lift);
        ui.painter().galley(rect.min, text, color);
        if lift > 0.0 {
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                (1.0, color.gamma_multiply(lift)),
            );
        }
        clicked = response.clicked();
    }
    ui.add_space(TITLE_BELOW);
    clicked
}

/// A section's title with a note at the right of its line: small capitals
/// that say something of the section and lead nowhere.
pub fn noted_title(state: &State, ui: &mut Ui, title: &str, note: &str) {
    let palette = &state.palette;
    ui.add_space(TITLE_ABOVE);
    let line = ui.available_rect_before_wrap();
    title_text(ui, title);
    let font = theme::bold(12.0);
    let text = widgets::tracked(ui, note, font, palette.secondary, 0.5, (f32::MAX, 1));
    let bottom = ui.min_rect().bottom() - 5.0;
    let at = pos2(line.right() - text.size().x, bottom - text.size().y);
    ui.painter().galley(at, text, palette.secondary);
    ui.add_space(TITLE_BELOW);
}

/// How many cards a line `room` wide holds when none may be narrower than
/// `least`, and how wide each then is: they fill the line exactly.
fn fitting(room: f32, least: f32) -> (usize, f32) {
    let count = ((room + GAP) / (least + GAP)).floor().max(1.0);
    (
        count as usize,
        ((room - GAP * (count - 1.0)) / count).floor(),
    )
}

/// A row of cards: as many as fit, never part of one, filling the row
/// from edge to edge. What does not fit is behind the section's "Show
/// all". `card` draws the one at an index.
pub fn row(ui: &mut Ui, count: usize, mut card: impl FnMut(&mut Ui, usize)) {
    let room = ui.available_width();
    let fits = fitting(room, ROW_CARD_MIN)
        .0
        .clamp(*ROW_CARDS.start(), *ROW_CARDS.end());
    let width = ((room - GAP * (fits as f32 - 1.0)) / fits as f32).floor();
    let (area, _) = ui.allocate_exact_size(vec2(room, height(width)), Sense::hover());
    if !ui.is_rect_visible(area) {
        return;
    }
    for index in 0..count.min(fits) {
        let left = area.left() + index as f32 * (width + GAP);
        place(ui, pos2(left, area.top()), width, index, &mut card);
    }
}

/// Every card, wrapping to as many lines as it takes: the page behind a
/// "Show all", and one kind of search result.
pub fn grid(ui: &mut Ui, count: usize, mut card: impl FnMut(&mut Ui, usize)) {
    let room = ui.available_width();
    let (columns, width) = fitting(room, GRID_CARD_MIN);
    let line = height(width) + GAP;
    let lines = count.div_ceil(columns);
    let size = vec2(room, (lines as f32 * line - GAP).max(0.0));
    let (area, _) = ui.allocate_exact_size(size, Sense::hover());
    // Only the lines in sight are drawn.
    let visible = ui.clip_rect();
    let first = ((visible.top() - area.top()) / line).floor().max(0.0) as usize;
    let last = ((visible.bottom() - area.top()) / line).ceil().max(0.0) as usize;
    for index in (first * columns)..count.min(last * columns) {
        let at = pos2(
            area.left() + (index % columns) as f32 * (width + GAP),
            area.top() + (index / columns) as f32 * line,
        );
        place(ui, at, width, index, &mut card);
    }
}

/// Draws the card at `index` in a place of its own, `width` wide.
fn place(
    ui: &mut Ui,
    at: egui::Pos2,
    width: f32,
    index: usize,
    card: &mut impl FnMut(&mut Ui, usize),
) {
    let rect = Rect::from_min_size(at, vec2(width, height(width)));
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("card", index))
            .max_rect(rect),
    );
    card(&mut cell, index);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_of_cards_fills_its_line_exactly() {
        // The page at the window's usual size: four cards and three gaps.
        let (count, width) = fitting(876.0, ROW_CARD_MIN);
        assert_eq!((count, width), (4, 207.0));
        assert_eq!(count as f32 * width + 3.0 * GAP, 876.0);
        // Too narrow for one at its least width: one, as wide as there is.
        assert_eq!(fitting(120.0, ROW_CARD_MIN), (1, 120.0));
    }
}
