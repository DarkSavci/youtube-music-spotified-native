//! The library, as the sidebar lists it: pinned things first, then folders,
//! then everything else, narrowed by the chips and by what is typed.

use std::collections::HashSet;

use eframe::egui::{self, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{Folder, LibraryItem, LibraryKind};

use super::drag;
use super::format::bulleted;
use super::widgets::{self, ArtShape};
use crate::actions::Action;
use crate::settings::LibrarySort;
use crate::state::{Loadable, Page, State};
use crate::theme::{self, Icon};

const ITEM_HEIGHT: f32 = 60.0;
/// The room kept beside the library's search field for the sort chip.
const SORT_CHIP_ROOM: f32 = 88.0;
const THUMB: f32 = 44.0;
/// How far the things in a folder sit to the right of it.
const NESTING: f32 = 18.0;
/// YouTube Music's own playlist of liked songs.
const LIKED_PLAYLIST: &str = "LM";

/// One line of the list.
#[derive(Debug, PartialEq)]
enum Row<'a> {
    Folder {
        folder: &'a Folder,
        /// How many things it holds.
        holds: usize,
        open: bool,
    },
    Item {
        item: &'a LibraryItem,
        /// Shown inside an open folder.
        nested: bool,
    },
}

/// How the list is narrowed and ordered.
struct View<'a> {
    kind: Option<LibraryKind>,
    query: &'a str,
    sort: LibrarySort,
    open_folders: &'a HashSet<String>,
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let items = match &state.library {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading(ui, palette, "Loading…");
            return;
        }
        Loadable::Failed(message) => {
            widgets::empty_state(ui, palette, Icon::Library, "Your library", message);
            import_offer(state, ui, actions);
            return;
        }
        Loadable::Loaded(items) => items,
    };
    tools(state, ui, actions);
    let view = View {
        kind: state.library_filter,
        query: &state.library_query,
        sort: state.settings.library_sort,
        open_folders: &state.open_folders,
    };
    let rows = arranged(items, &state.folders, &view);
    if rows.is_empty() {
        let (icon, title, text) = if state.library_query.is_empty() {
            (
                Icon::Library,
                "Nothing here yet",
                "Playlists, albums and artists you save appear here.",
            )
        } else {
            (
                Icon::Search,
                "Nothing matches",
                "Try fewer letters, or another kind.",
            )
        };
        widgets::empty_state(ui, palette, icon, title, text);
        return;
    }
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
                    Row::Item { item, nested } => item_row(state, ui, actions, item, *nested),
                }
            }
        });
}

/// The library's own search field and its sort order.
fn tools(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let sort = state.settings.library_sort;
        // The field takes what the sort chip leaves.
        let field = widgets::TextField {
            text: &state.library_query,
            hint: "Search in library",
            label: "Search in library",
            icon: Some(Icon::Search),
            width: (ui.available_width() - SORT_CHIP_ROOM).max(80.0),
        };
        if let Some(query) = field.show(ui, palette) {
            actions.push(Action::SetLibraryQuery(query));
        }
        if widgets::chip(ui, palette, sort.label(), false)
            .on_hover_text("Change the order")
            .clicked()
        {
            actions.push(Action::CycleLibrarySort);
        }
    });
    ui.add_space(8.0);
}

/// The lines to show. Looking for something (a chip is on, or something is
/// typed) lists every match flat; otherwise folders hold what was put in
/// them and show it when open.
fn arranged<'a>(items: &'a [LibraryItem], folders: &'a [Folder], view: &View) -> Vec<Row<'a>> {
    let query = view.query.trim().to_lowercase();
    let mut matching: Vec<&LibraryItem> = items
        .iter()
        .filter(|item| view.kind.is_none_or(|kind| item.kind == kind))
        .filter(|item| query.is_empty() || item.title.to_lowercase().contains(&query))
        .collect();
    // Both sorts are stable, so pinned things keep the chosen order among
    // themselves, as do the rest.
    if view.sort == LibrarySort::Name {
        matching.sort_by_cached_key(|item| item.title.to_lowercase());
    }
    matching.sort_by_key(|item| !item.pinned);

    let flat = |item| Row::Item {
        item,
        nested: false,
    };
    let searching = view.kind.is_some() || !query.is_empty();
    if searching || folders.is_empty() {
        return matching.into_iter().map(flat).collect();
    }
    let in_folder = |item: &LibraryItem, folder: &Folder| item.folder_id == folder.id;
    // A thing filed in a folder that no longer exists is at the top level.
    let loose = |item: &&LibraryItem| !folders.iter().any(|folder| in_folder(item, folder));
    let mut rows: Vec<Row> = matching
        .iter()
        .copied()
        .filter(|item| item.pinned)
        .filter(loose)
        .map(flat)
        .collect();
    for folder in folders {
        let held = matching.iter().filter(|item| in_folder(item, folder));
        let open = view.open_folders.contains(&folder.id);
        rows.push(Row::Folder {
            folder,
            holds: held.clone().count(),
            open,
        });
        if open {
            rows.extend(held.map(|&item| Row::Item { item, nested: true }));
        }
    }
    rows.extend(
        matching
            .iter()
            .copied()
            .filter(|item| !item.pinned)
            .filter(loose)
            .map(flat),
    );
    rows
}

/// The ways to sign in, under the note that says the library needs it.
fn import_offer(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        if state.signing_in {
            widgets::spinner(ui, palette, 18.0);
        } else {
            if widgets::pill_button(ui, palette, "Sign in").clicked() {
                actions.push(Action::SignIn);
            }
            if state.import_source.is_some() {
                ui.add_space(6.0);
                if widgets::outline_button(ui, palette, "Import sign-in").clicked() {
                    actions.push(Action::ImportSignIn);
                }
            }
        }
        ui.add_space(6.0);
        let (note, color) = match &state.import_error {
            Some(error) => (error.as_str(), palette.danger),
            None if state.signing_in => ("Sign in in the browser window.", palette.secondary),
            None => ("", palette.dim),
        };
        if !note.is_empty() {
            ui.label(
                egui::RichText::new(note)
                    .font(theme::regular(12.0))
                    .color(color),
            );
        }
    });
}

/// Fills a row that is open, and washes one under the pointer.
fn highlight(state: &State, ui: &Ui, rect: Rect, open: bool, response: &egui::Response) {
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
    let font = theme::regular(12.5);
    let second = widgets::elided(ui, lines.1, font, palette.secondary, width, 1);
    let at = pos2(second_left, rect.center().y + 2.0);
    ui.painter().galley(at, second, palette.secondary);
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
    response.context_menu(|ui| {
        if ui.button("Delete folder").clicked() {
            actions.push(Action::DeleteFolder(folder.id.clone()));
            ui.close();
        }
    });
    highlight(state, ui, rect, false, &response);
    let thumb = Rect::from_center_size(
        pos2(rect.left() + 8.0 + THUMB / 2.0, rect.center().y),
        vec2(THUMB, THUMB),
    );
    ui.painter()
        .rect_filled(thumb, theme::RADIUS_ROW, palette.surface);
    widgets::paint_icon(ui, Icon::Folder, thumb, 20.0, palette.secondary);
    let chevron =
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(16.0, 16.0));
    let icon = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    widgets::paint_icon(ui, icon, chevron, 16.0, palette.secondary);
    let holds = match holds {
        0 => "Empty".to_owned(),
        1 => "1 item".to_owned(),
        holds => format!("{holds} items"),
    };
    let text_rect = rect.with_max_x(chevron.left());
    let second = bulleted(["Folder", &holds]);
    captions(
        state,
        ui,
        text_rect,
        thumb.right() + 12.0,
        (&folder.name, &second),
        false,
    );
    if response.clicked() {
        actions.push(Action::ToggleFolder(folder.id.clone()));
    }
}

/// What a library item is, where it leads, and how it is drawn.
fn item_kind(item: &LibraryItem) -> (Option<Page>, &'static str, ArtShape, Icon) {
    let rounded = ArtShape::Rounded(theme::RADIUS_ROW);
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
    response.context_menu(|ui| item_menu(state, ui, actions, item));
    let (page, kind, shape, placeholder) = item_kind(item);
    // Songs dropped on Liked Music are liked; on a playlist, added to it.
    if item.kind == LibraryKind::Playlist
        && let Some(tracks) = drag::target(state, ui, &response)
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
    highlight(state, ui, rect, open, &response);
    let indent = if nested { NESTING } else { 0.0 };
    let thumb = Rect::from_center_size(
        pos2(rect.left() + 8.0 + indent + THUMB / 2.0, rect.center().y),
        vec2(THUMB, THUMB),
    );
    widgets::artwork(ui, state, &item.artwork, thumb, shape, placeholder);
    let second = bulleted([kind, &item.subtitle]);
    captions(
        state,
        ui,
        rect,
        thumb.right() + 12.0,
        (&item.title, &second),
        item.pinned,
    );
    if response.clicked()
        && let Some(page) = page
    {
        actions.push(Action::Open(page));
    }
}

/// What can be done with a thing in the library: keep it at the top, file
/// it in a folder, and for a playlist of the person's own, delete it.
fn item_menu(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, item: &LibraryItem) {
    ui.set_min_width(190.0);
    let mut chosen = None;
    let pin = if item.pinned { "Unpin" } else { "Pin" };
    if ui.button(pin).clicked() {
        chosen = Some(Action::SetPinned {
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
    ui.menu_button("Move to folder", |ui| {
        ui.set_min_width(180.0);
        if ui.button("New folder…").clicked() {
            chosen = Some(Action::NewFolder);
        }
        if !state.folders.is_empty() {
            ui.separator();
        }
        for folder in &state.folders {
            let here = folder.id == item.folder_id;
            if ui
                .add_enabled(!here, egui::Button::new(&folder.name))
                .clicked()
            {
                chosen = Some(move_to(&folder.id));
            }
        }
    });
    if !item.folder_id.is_empty() && ui.button("Remove from folder").clicked() {
        chosen = Some(move_to(""));
    }
    // Liked Music is YouTube's own and cannot be deleted.
    if item.kind == LibraryKind::Playlist && item.id != LIKED_PLAYLIST {
        ui.separator();
        if ui.button("Delete playlist").clicked() {
            chosen = Some(Action::AskDeletePlaylist {
                playlist_id: item.id.clone(),
                title: item.title.clone(),
            });
        }
    }
    if let Some(action) = chosen {
        actions.push(action);
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, kind: LibraryKind, pinned: bool) -> LibraryItem {
        LibraryItem {
            title: title.into(),
            kind,
            pinned,
            ..LibraryItem::default()
        }
    }

    fn filed(title: &str, folder: &str) -> LibraryItem {
        LibraryItem {
            folder_id: folder.into(),
            ..item(title, LibraryKind::Playlist, false)
        }
    }

    fn view<'a>(
        kind: Option<LibraryKind>,
        query: &'a str,
        sort: LibrarySort,
        open_folders: &'a HashSet<String>,
    ) -> View<'a> {
        View {
            kind,
            query,
            sort,
            open_folders,
        }
    }

    /// Each row as a word: a folder as `[name]`, a nested item indented.
    fn titles(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Folder { folder, .. } => format!("[{}]", folder.name),
                Row::Item { item, nested: true } => format!("  {}", item.title),
                Row::Item { item, .. } => item.title.clone(),
            })
            .collect()
    }

    #[test]
    fn the_library_is_narrowed_by_kind_and_by_what_is_typed() {
        let library = [
            item("Road trip", LibraryKind::Playlist, false),
            item("Discovery", LibraryKind::Album, false),
            item("Roads", LibraryKind::Album, false),
        ];
        let none = HashSet::new();
        let albums = view(Some(LibraryKind::Album), "", LibrarySort::Recent, &none);
        assert_eq!(
            titles(&arranged(&library, &[], &albums)),
            ["Discovery", "Roads"]
        );
        let typed = view(None, " ROAD ", LibrarySort::Recent, &none);
        assert_eq!(
            titles(&arranged(&library, &[], &typed)),
            ["Road trip", "Roads"]
        );
    }

    #[test]
    fn pinned_items_come_first_in_either_order() {
        let library = [
            item("Zebra", LibraryKind::Playlist, false),
            item("Liked Music", LibraryKind::Playlist, true),
            item("apple", LibraryKind::Playlist, false),
        ];
        let none = HashSet::new();
        let by_name = view(None, "", LibrarySort::Name, &none);
        assert_eq!(
            titles(&arranged(&library, &[], &by_name)),
            ["Liked Music", "apple", "Zebra"]
        );
        let recent = view(None, "", LibrarySort::Recent, &none);
        assert_eq!(
            titles(&arranged(&library, &[], &recent)),
            ["Liked Music", "Zebra", "apple"]
        );
    }

    #[test]
    fn a_folder_holds_what_was_filed_in_it_and_shows_it_when_open() {
        let library = [
            filed("Running", "f1"),
            item("Liked Music", LibraryKind::Playlist, true),
            item("Road trip", LibraryKind::Playlist, false),
            filed("Lost", "gone"),
        ];
        let folders = [Folder {
            id: "f1".into(),
            name: "Sport".into(),
        }];
        let shut = HashSet::new();
        let closed = view(None, "", LibrarySort::Recent, &shut);
        assert_eq!(
            titles(&arranged(&library, &folders, &closed)),
            ["Liked Music", "[Sport]", "Road trip", "Lost"]
        );
        let opened = HashSet::from(["f1".to_owned()]);
        let open = view(None, "", LibrarySort::Recent, &opened);
        assert_eq!(
            titles(&arranged(&library, &folders, &open)),
            ["Liked Music", "[Sport]", "  Running", "Road trip", "Lost"]
        );
    }

    #[test]
    fn looking_for_something_finds_it_inside_a_shut_folder() {
        let library = [filed("Running", "f1")];
        let folders = [Folder {
            id: "f1".into(),
            name: "Sport".into(),
        }];
        let shut = HashSet::new();
        let typed = view(None, "run", LibrarySort::Recent, &shut);
        assert_eq!(titles(&arranged(&library, &folders, &typed)), ["Running"]);
    }
}
