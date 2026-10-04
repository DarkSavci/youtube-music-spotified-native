//! The library: how the sidebar shows it, and keeping it in order: what
//! is pinned to the top, and what is filed in which folder. Pins and
//! folders do not exist in YouTube Music; the core keeps both on this
//! computer.

use spotified_client::models::{LibraryItem, LibraryKind};

use super::{Action, Effect};
use crate::backend::Request;
use crate::state::{Dialog, Loadable, State};

/// Each change shows at once; the core's answer only matters if it is a no,
/// in which case the library is fetched again to show what is really so.
pub(super) fn organise(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::ToggleSidebar => {
            state.settings.sidebar_collapsed = !state.settings.sidebar_collapsed;
            // The rail has no search field: what was typed would go on
            // hiding things with no way in sight to clear it.
            if state.settings.sidebar_collapsed {
                state.library_query.clear();
            }
            vec![Effect::SaveSettings]
        }
        Action::ToggleLibraryExpanded => {
            state.library_expanded = !state.library_expanded;
            Vec::new()
        }
        // The wide library and the sidebar each remember their own way.
        Action::ToggleLibraryGrid => {
            let grid = if state.library_expanded {
                &mut state.settings.library_expanded_grid
            } else {
                &mut state.settings.library_grid
            };
            *grid = !*grid;
            vec![Effect::SaveSettings]
        }
        Action::FilterLibrary(kind) => {
            state.library_filter = (state.library_filter != Some(kind)).then_some(kind);
            Vec::new()
        }
        Action::SetLibraryQuery(query) => {
            state.library_query = query;
            Vec::new()
        }
        Action::SetLibrarySort(sort) => {
            if state.settings.library_sort == sort {
                return Vec::new();
            }
            state.settings.library_sort = sort;
            vec![Effect::SaveSettings]
        }
        Action::SetPinned {
            kind,
            item_id,
            pinned,
        } => {
            let Some(item) = find(state, kind, &item_id) else {
                return Vec::new();
            };
            item.pinned = pinned;
            vec![Effect::Fetch(Request::Organise {
                kind,
                item_id,
                pinned: Some(pinned),
                folder_id: None,
            })]
        }
        Action::MoveToFolder {
            kind,
            item_id,
            folder_id,
        } => {
            let Some(item) = find(state, kind, &item_id) else {
                return Vec::new();
            };
            item.folder_id.clone_from(&folder_id);
            // Open, so what was just filed can be seen to have arrived.
            if !folder_id.is_empty() {
                state.open_folders.insert(folder_id.clone());
            }
            vec![Effect::Fetch(Request::Organise {
                kind,
                item_id,
                pinned: None,
                folder_id: Some(folder_id),
            })]
        }
        Action::NewFolder => {
            state.dialog = Some(Dialog::NewFolder {
                name: String::new(),
            });
            Vec::new()
        }
        Action::DeleteFolder(folder_id) => {
            state.folders.retain(|folder| folder.id != folder_id);
            state.open_folders.remove(&folder_id);
            vec![Effect::Fetch(Request::DeleteFolder(folder_id))]
        }
        Action::ToggleFolder(folder_id) => {
            if !state.open_folders.remove(&folder_id) {
                state.open_folders.insert(folder_id);
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn find<'a>(state: &'a mut State, kind: LibraryKind, id: &str) -> Option<&'a mut LibraryItem> {
    let Loadable::Loaded(library) = &mut state.library else {
        return None;
    };
    library
        .iter_mut()
        .find(|item| item.kind == kind && item.id == id)
}
