//! Keeping the library in order: what is pinned to the top, and what is
//! filed in which folder. Neither exists in YouTube Music; the core keeps
//! both on this computer.

use spotified_client::models::{LibraryItem, LibraryKind};

use super::{Action, Effect};
use crate::backend::Request;
use crate::state::{Dialog, Loadable, State};

/// Each change shows at once; the core's answer only matters if it is a no,
/// in which case the library is fetched again to show what is really so.
pub(super) fn organise(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
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
