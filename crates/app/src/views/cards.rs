//! Cards and the shelves that hold them.

use eframe::egui::{self, Align, Layout, Rect, Response, Sense, Ui, pos2, vec2};
use spotified_client::models::{
    Album, Artist, Artwork, Item, Mix, Playlist, Podcast, Track, join_names,
};

use super::format::bulleted;
use super::menus;
use super::widgets::{self, ArtShape};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon};

const WIDTH: f32 = 172.0;
const PADDING: f32 = 12.0;
const IMAGE: f32 = WIDTH - PADDING * 2.0;
const TITLE_HEIGHT: f32 = 18.0;
/// Two lines are reserved whether or not the subtitle fills them, so a row
/// of cards keeps one baseline.
const SUBTITLE_HEIGHT: f32 = 34.0;
const HEIGHT: f32 = PADDING + IMAGE + 10.0 + TITLE_HEIGHT + 2.0 + SUBTITLE_HEIGHT + PADDING;
const SHELF_GAP: f32 = 7.0;
const PLAY_BUTTON: f32 = 44.0;
/// How far below its place the play button starts as it fades in.
const PLAY_RISE: f32 = 8.0;
/// How far in from the cover's bottom right corner the play button's
/// centre sits.
const PLAY_INSET: f32 = 26.0;

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

fn card(state: &State, ui: &mut Ui, card: Card<'_>) -> CardResponse {
    let palette = &state.palette;
    let (rect, response) = ui.allocate_exact_size(vec2(WIDTH, HEIGHT), Sense::click());
    widgets::name(ui, &response, card.title);
    if !ui.is_rect_visible(rect) {
        return CardResponse {
            card: response,
            play: false,
        };
    }
    let image = Rect::from_min_size(rect.min + vec2(PADDING, PADDING), vec2(IMAGE, IMAGE));
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
        ui.painter()
            .rect_filled(rect, theme::RADIUS, widgets::wash(ui, lift));
    }
    widgets::artwork(ui, state, card.art, image, card.shape, card.placeholder);

    let title_top = image.bottom() + 10.0;
    let title = widgets::elided(
        ui,
        card.title,
        theme::semibold(14.0),
        palette.text,
        IMAGE,
        1,
    );
    ui.painter()
        .galley(pos2(image.left(), title_top), title, palette.text);
    let subtitle = widgets::elided(
        ui,
        &card.subtitle,
        theme::regular(12.5),
        palette.secondary,
        IMAGE,
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

/// The menu on an album, a playlist or an artist.
fn collection_menu(ui: &mut Ui, actions: &mut Vec<Action>, page: &Page, link: String) {
    ui.set_min_width(200.0);
    let mut chosen = None;
    if ui.button("Play").clicked() {
        chosen = Some(Action::PlayCollection(page.clone()));
    }
    if ui.button("Open").clicked() {
        chosen = Some(Action::Open(page.clone()));
    }
    ui.separator();
    if ui.button("Copy link").clicked() {
        chosen = Some(Action::CopyLink(link));
    }
    if let Some(action) = chosen {
        actions.push(action);
        ui.close();
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
    let response = card(state, ui, card_spec);
    response
        .card
        .context_menu(|ui| collection_menu(ui, actions, &page, link));
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
    let artists = join_names(&album.artists);
    let spec = Card {
        art: &album.artwork,
        shape: ArtShape::Rounded(theme::RADIUS_ROW),
        placeholder: Icon::Music,
        title: &album.title,
        subtitle: bulleted([album.year.as_str(), kind, &artists]),
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
    let spec = Card {
        art: &artist.artwork,
        shape: ArtShape::Circle,
        placeholder: Icon::User,
        title: &artist.name,
        subtitle: "Artist".to_owned(),
        playable: true,
    };
    let link = format!("https://music.youtube.com/channel/{}", artist.id);
    collection_card(
        state,
        ui,
        actions,
        spec,
        Page::Artist(artist.id.clone()),
        link,
    );
}

pub fn playlist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, playlist: &Playlist) {
    let spec = Card {
        art: &playlist.artwork,
        shape: ArtShape::Rounded(theme::RADIUS_ROW),
        placeholder: Icon::ListMusic,
        title: &playlist.title,
        subtitle: bulleted(["Playlist", &playlist.owner]),
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
        shape: ArtShape::Rounded(theme::RADIUS_ROW),
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
        shape: ArtShape::Rounded(theme::RADIUS_ROW),
        placeholder: Icon::MicVocal,
        title: &podcast.title,
        subtitle: bulleted(["Podcast", &podcast.author]),
        playable: true,
    };
    let link = format!("https://music.youtube.com/playlist?list={}", podcast.id);
    let page = Page::Podcast(podcast.id.clone());
    collection_card(state, ui, actions, spec, page, link);
}

/// A song shown as a card. Clicking it plays it; the core carries on with
/// similar songs when it ends.
pub fn track(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, track: &Track) {
    playable_card(state, ui, actions, track, "Song");
}

/// A song or an episode: something a click plays. `kind` says which.
fn playable_card(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, track: &Track, kind: &str) {
    let response = card(
        state,
        ui,
        Card {
            art: &track.artwork,
            shape: ArtShape::Rounded(theme::RADIUS_ROW),
            placeholder: Icon::Music,
            title: &track.title,
            subtitle: bulleted([kind, &track.artist_names()]),
            playable: track.playable,
        },
    );
    response
        .card
        .context_menu(|ui| menus::tracks(state, ui, actions, &[track], None));
    if (response.play || response.card.clicked()) && track.playable {
        actions.push(Action::Play {
            tracks: vec![track.clone()],
            index: 0,
            origin: track.title.clone(),
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
        Item::Episode(value) => playable_card(state, ui, actions, value, "Episode"),
    }
}

pub fn section_title(ui: &mut Ui, title: &str) {
    ui.add_space(18.0);
    ui.label(egui::RichText::new(title).font(theme::bold(17.0)));
    ui.add_space(4.0);
}

/// A section's title, with a way to the whole of it when the section
/// shows only a part.
pub fn linked_title(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    title: &str,
    whole: Option<Page>,
) {
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(theme::bold(17.0)));
        let Some(whole) = whole else {
            return;
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let text = egui::RichText::new("Show all")
                .font(theme::semibold(12.5))
                .color(state.palette.secondary);
            let link = ui.add(egui::Label::new(text).sense(Sense::click()));
            if link.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if link.clicked() {
                actions.push(Action::Open(whole));
            }
        });
    });
    ui.add_space(4.0);
}

/// A titled row of cards that scrolls sideways. `id` tells one shelf's
/// scroll position from another's; `cards` draws them.
pub fn shelf(
    ui: &mut Ui,
    title: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    cards: impl FnOnce(&mut Ui),
) {
    section_title(ui, title);
    row(ui, id, cards);
}

/// A row of cards that scrolls sideways.
pub fn row(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, cards: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::horizontal()
        .id_salt(("shelf", id))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SHELF_GAP;
                cards(ui);
            });
        });
}
