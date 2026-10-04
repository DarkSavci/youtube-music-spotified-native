//! The library, as the sidebar lists it: pinned things first, then folders,
//! then everything else, narrowed by the chips and by what is typed. Shown
//! as rows, as a grid of covers, or as a rail of covers alone.

mod arrange;
mod grid;

use eframe::egui::{self, Frame, Margin, Rect, Response, Sense, Ui, pos2, vec2};
use spotified_client::models::{Folder, LibraryItem, LibraryKind};

use super::drag;
use super::format::middle_dotted;
use super::widgets::menu::{self, Entry, Menu};
use super::widgets::{self, ArtShape};
use crate::actions::Action;
use crate::state::{Loadable, Page, State};
use crate::theme::{self, Icon};
use arrange::{Row, View, arranged};

const ITEM_HEIGHT: f32 = 64.0;
const THUMB: f32 = 48.0;
/// How far the things in a folder sit to the right of it.
const NESTING: f32 = 18.0;
/// YouTube Music's own playlist of liked songs.
const LIKED_PLAYLIST: &str = "LM";

/// How the library is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    /// A row each: cover, title, and what it is.
    List,
    /// Covers in columns at least this wide.
    Grid(f32),
    /// Covers alone, for the collapsed sidebar.
    Rail,
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, layout: Layout) {
    let palette = &state.palette;
    let items = match &state.library {
        Loadable::NotLoaded | Loadable::Loading => {
            if layout != Layout::Rail {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    widgets::loading(ui, palette, "Loading…");
                });
            }
            return;
        }
        Loadable::Failed(message) => {
            if layout != Layout::Rail {
                unavailable(state, ui, actions, message);
            }
            return;
        }
        Loadable::Loaded(items) => items,
    };
    let view = View {
        kind: state.library_filter,
        query: &state.library_query,
        sort: state.settings.library_sort,
        open_folders: &state.open_folders,
    };
    let rows = arranged(items, &state.folders, &view);
    if rows.is_empty() {
        if layout == Layout::Rail {
            return;
        }
        let (title, text) = if state.library_query.is_empty() {
            (
                "Nothing saved yet",
                "Albums, artists and playlists you save will appear here.",
            )
        } else {
            (
                "No matches in your library",
                "Try fewer letters, or another kind.",
            )
        };
        note(state, ui, title, text, |_| {});
        return;
    }
    match layout {
        Layout::List => {
            egui::ScrollArea::vertical()
                .id_salt("library")
                .auto_shrink([false, false])
                .show_rows(ui, ITEM_HEIGHT, rows.len(), |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for row in &rows[range] {
                        match row {
                            Row::Folder {
                                folder,
                                holds,
                                open,
                            } => folder_row(state, ui, actions, folder, *holds, *open),
                            Row::Item { item, nested } => {
                                item_row(state, ui, actions, item, *nested);
                            }
                        }
                    }
                });
        }
        Layout::Grid(least) => grid::grid(state, ui, actions, &rows, least),
        Layout::Rail => grid::rail(state, ui, actions, &rows),
    }
}

/// A note in a box of its own, as the Electron app shows a library with
/// nothing in it: a title, a line, and whatever `more` adds under them.
fn note(state: &State, ui: &mut Ui, title: &str, text: &str, more: impl FnOnce(&mut Ui)) {
    let palette = &state.palette;
    Frame::new()
        .fill(palette.surface)
        .corner_radius(theme::RADIUS)
        .outer_margin(Margin::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(egui::RichText::new(title).font(theme::bold(14.0)));
            ui.label(
                egui::RichText::new(text)
                    .font(theme::regular(12.0))
                    .color(palette.secondary),
            );
            more(ui);
        });
}

/// The library could not be had: why, and the ways to sign in when that
/// is why.
fn unavailable(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, message: &str) {
    let palette = &state.palette;
    let signed_out = state.account.is_none();
    let (title, text) = match (&state.import_error, signed_out, state.signing_in) {
        (Some(error), ..) => ("Signed out", error.as_str()),
        (None, true, true) => (
            "Signed out",
            "Sign in in the browser window. It closes by itself once you are in.",
        ),
        (None, true, false) => ("Signed out", "Sign in to see your library."),
        (None, false, _) => ("Could not load your library", message),
    };
    note(state, ui, title, text, |ui| {
        if !signed_out {
            return;
        }
        if state.signing_in {
            widgets::spinner(ui, palette, 18.0);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if widgets::chip(ui, palette, "Sign in", false).clicked() {
                actions.push(Action::SignIn);
            }
            // The earlier app is on this computer, signed in or not.
            if state.migration.found.is_some()
                && widgets::chip(ui, palette, "Move from the old app", false).clicked()
            {
                actions.push(Action::OpenMigration);
            }
        });
    });
}

/// Fills a row that is open, and washes one under the pointer.
fn highlight(state: &State, ui: &Ui, rect: Rect, open: bool, response: &Response) {
    if open {
        let fill = state.palette.surface_active;
        ui.painter().rect_filled(rect, theme::RADIUS_ROW, fill);
    }
    widgets::row_hover(ui, response, rect);
}

/// A row's two lines of text, to the right of its picture. `pinned` puts
/// a pin before the second.
fn captions(state: &State, ui: &Ui, rect: Rect, left: f32, lines: (&str, &str), pinned: bool) {
    let palette = &state.palette;
    let width = (rect.right() - left - 8.0).max(20.0);
    let name = widgets::elided(ui, lines.0, theme::medium(14.0), palette.text, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y - 18.0), name, palette.text);
    let mut second_left = left;
    if pinned {
        let pin = Rect::from_min_size(pos2(left, rect.center().y + 3.0), vec2(13.0, 13.0));
        widgets::paint_icon(ui, Icon::Pin, pin, 13.0, palette.accent);
        second_left += 17.0;
    }
    let width = (rect.right() - second_left - 8.0).max(20.0);
    let font = theme::regular(12.0);
    let second = widgets::elided(ui, lines.1, font, palette.secondary, width, 1);
    let at = pos2(second_left, rect.center().y + 2.0);
    ui.painter().galley(at, second, palette.secondary);
}

/// "Empty", "1 item", "3 items".
fn holding(holds: usize) -> String {
    match holds {
        0 => "Empty".to_owned(),
        1 => "1 item".to_owned(),
        holds => format!("{holds} items"),
    }
}

/// What a folder answers to, however it is drawn: a click opens or shuts
/// it, and its menu deletes it.
fn folder_interact(
    palette: &theme::Palette,
    actions: &mut Vec<Action>,
    response: &Response,
    folder: &Folder,
) {
    menu::context(response, palette, |menu| {
        if menu.entry(Entry::new("Delete folder").danger()) {
            actions.push(Action::DeleteFolder(folder.id.clone()));
        }
    });
    if response.clicked() {
        actions.push(Action::ToggleFolder(folder.id.clone()));
    }
}

fn folder_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    folder: &Folder,
    holds: usize,
    open: bool,
) {
    let palette = &state.palette;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ITEM_HEIGHT), Sense::click());
    widgets::name(ui, &response, &folder.name);
    folder_interact(&state.palette, actions, &response, folder);
    highlight(state, ui, rect, false, &response);
    let thumb = Rect::from_center_size(
        pos2(rect.left() + 8.0 + THUMB / 2.0, rect.center().y),
        vec2(THUMB, THUMB),
    );
    ui.painter().rect_filled(thumb, 4.0, palette.surface);
    widgets::paint_icon(ui, Icon::Folder, thumb, 20.0, palette.secondary);
    let chevron =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(16.0, 16.0));
    let icon = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    widgets::paint_icon(ui, icon, chevron, 16.0, palette.secondary);
    let text_rect = rect.with_max_x(chevron.left());
    let second = middle_dotted(["Folder", &holding(holds)]);
    captions(
        state,
        ui,
        text_rect,
        thumb.right() + 12.0,
        (&folder.name, &second),
        false,
    );
}

/// What a library item is, where it leads, and how it is drawn.
fn item_kind(item: &LibraryItem) -> (Option<Page>, &'static str, ArtShape, Icon) {
    let rounded = ArtShape::Rounded(4);
    let id = item.id.clone();
    match item.kind {
        LibraryKind::Playlist => (
            Some(Page::Playlist(id)),
            "Playlist",
            rounded,
            Icon::ListMusic,
        ),
        LibraryKind::Album => (Some(Page::Album(id)), "Album", rounded, Icon::Music),
        LibraryKind::Artist => (
            Some(Page::Artist(id)),
            "Artist",
            ArtShape::Circle,
            Icon::User,
        ),
        LibraryKind::Podcast => (Some(Page::Podcast(id)), "Podcast", rounded, Icon::MicVocal),
    }
}

/// What a library item answers to, however it is drawn: a click opens it,
/// a double click plays it, songs dropped on a playlist are added to it,
/// and the right button brings its menu. Returns whether it is the page
/// on screen.
fn item_interact(
    state: &State,
    ui: &Ui,
    actions: &mut Vec<Action>,
    response: &Response,
    item: &LibraryItem,
) -> bool {
    menu::context(response, &state.palette, |menu| {
        item_menu(state, menu, actions, item);
    });
    let (page, ..) = item_kind(item);
    // Songs dropped on Liked Music are liked; on a playlist, added to it.
    if item.kind == LibraryKind::Playlist
        && let Some(tracks) = drag::target(state, ui, response)
    {
        actions.push(if item.id == LIKED_PLAYLIST {
            Action::LikeAll(tracks)
        } else {
            Action::AddToPlaylist {
                playlist_id: item.id.clone(),
                playlist_title: item.title.clone(),
                track_ids: tracks.into_iter().map(|track| track.id).collect(),
            }
        });
    }
    let open = page.as_ref() == Some(state.nav.page());
    if let Some(page) = page {
        if response.double_clicked() && playable(item) {
            actions.push(Action::PlayCollection(page));
        } else if response.clicked() {
            actions.push(Action::Open(page));
        }
    }
    open
}

/// A show plays episode by episode; the other kinds play as a whole.
fn playable(item: &LibraryItem) -> bool {
    item.kind != LibraryKind::Podcast
}

fn item_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    item: &LibraryItem,
    nested: bool,
) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ITEM_HEIGHT), Sense::click());
    widgets::name(ui, &response, &item.title);
    let open = item_interact(state, ui, actions, &response, item);
    let (_, kind, shape, placeholder) = item_kind(item);
    highlight(state, ui, rect, open, &response);
    let indent = if nested { NESTING } else { 0.0 };
    let thumb = Rect::from_center_size(
        pos2(rect.left() + 8.0 + indent + THUMB / 2.0, rect.center().y),
        vec2(THUMB, THUMB),
    );
    widgets::artwork(ui, state, &item.artwork, thumb, shape, placeholder);
    play_over(ui, actions, (&response, rect), (thumb, shape), item);
    let second = middle_dotted([kind, &item.subtitle]);
    captions(
        state,
        ui,
        rect,
        thumb.right() + 12.0,
        (&item.title, &second),
        item.pinned,
    );
}

/// The play button that covers a row's picture while the pointer is on
/// the row, so starting a playlist does not take a trip through its page.
fn play_over(
    ui: &Ui,
    actions: &mut Vec<Action>,
    (row, row_rect): (&Response, Rect),
    (thumb, shape): (Rect, ArtShape),
    item: &LibraryItem,
) {
    let (Some(page), true) = (item_kind(item).0, playable(item)) else {
        return;
    };
    // The button lies over the row, which would lose its hover to it; the
    // pointer being anywhere in the row is what counts.
    let inside = ui.rect_contains_pointer(row_rect);
    let lift = widgets::hover_of(ui, row.id.with("play-over"), inside);
    if lift <= 0.0 {
        return;
    }
    let radius = match shape {
        ArtShape::Rounded(radius) => egui::CornerRadius::same(radius),
        ArtShape::Circle => egui::CornerRadius::same(u8::MAX),
    };
    let shade = egui::Color32::from_black_alpha((128.0 * lift) as u8);
    ui.painter().rect_filled(thumb, radius, shade);
    let glyph = egui::Color32::WHITE.gamma_multiply(lift);
    widgets::paint_icon(ui, Icon::PlayFilled, thumb, 20.0, glyph);
    if !inside {
        return;
    }
    let button = ui.interact(thumb, row.id.with("play"), Sense::click());
    widgets::name(ui, &button, &format!("Play {}", item.title));
    if button.clicked() {
        actions.push(Action::PlayCollection(page));
    }
}

/// What can be done with a thing in the library: play it, keep it at the
/// top, file it in a folder, and for a playlist of the person's own,
/// delete it.
fn item_menu(state: &State, menu: &mut Menu<'_>, actions: &mut Vec<Action>, item: &LibraryItem) {
    if let (Some(page), true) = (item_kind(item).0, playable(item)) {
        if menu.entry(Entry::new("Play").named("Play this")) {
            actions.push(Action::PlayCollection(page));
        }
        menu.separator();
    }
    let pin = if item.pinned { "Unpin" } else { "Pin to top" };
    if menu.item(pin) {
        actions.push(Action::SetPinned {
            kind: item.kind,
            item_id: item.id.clone(),
            pinned: !item.pinned,
        });
    }
    let move_to = |folder_id: &str| Action::MoveToFolder {
        kind: item.kind,
        item_id: item.id.clone(),
        folder_id: folder_id.to_owned(),
    };
    menu.separator();
    menu.submenu(Entry::new("Move to folder"), |menu| {
        if menu.entry(Entry::new("New folder…").icon(Icon::Plus)) {
            actions.push(Action::NewFolder);
        }
        menu.separator();
        for folder in &state.folders {
            let here = folder.id == item.folder_id;
            let entry = Entry::new(&folder.name).icon(Icon::Folder);
            if menu.entry(entry.enabled(!here).checked(here)) {
                actions.push(move_to(&folder.id));
            }
        }
    });
    if !item.folder_id.is_empty() && menu.item("Remove from folder") {
        actions.push(move_to(""));
    }
    // Liked Music is YouTube's own and cannot be deleted.
    if item.kind == LibraryKind::Playlist && item.id != LIKED_PLAYLIST {
        menu.separator();
        if menu.entry(Entry::new("Delete playlist").danger()) {
            actions.push(Action::AskDeletePlaylist {
                playlist_id: item.id.clone(),
                title: item.title.clone(),
            });
        }
    }
}
