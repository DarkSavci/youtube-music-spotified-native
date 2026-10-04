//! Which of the library's things are listed, and in what order: narrowed
//! by the chips and by what is typed, sorted, pinned things first, and
//! folders holding what was filed in them.

use std::cmp::Ordering;
use std::collections::HashSet;

use spotified_client::models::{Folder, LibraryItem, LibraryKind};

use crate::settings::LibrarySort;

/// One line of the list.
#[derive(Debug, PartialEq)]
pub enum Row<'a> {
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
pub struct View<'a> {
    pub kind: Option<LibraryKind>,
    pub query: &'a str,
    pub sort: LibrarySort,
    pub open_folders: &'a HashSet<String>,
}

/// A time as the core writes it (RFC 3339), in a form that compares in
/// order: the seconds, then the fraction of one, which the core writes
/// with as many digits as it has.
fn moment(time: &str) -> (&str, &str) {
    let seconds = time.get(..19).unwrap_or(time);
    let fraction = time[seconds.len()..].strip_prefix('.').unwrap_or("");
    let digits = fraction
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(fraction.len());
    (seconds, &fraction[..digits])
}

/// Newest first; a thing with no time recorded comes after all that have
/// one. `None` when the times cannot tell the two apart.
fn newest_first(a: Option<&String>, b: Option<&String>) -> Option<Ordering> {
    match (a, b) {
        (None, None) => None,
        (None, Some(_)) => Some(Ordering::Greater),
        (Some(_), None) => Some(Ordering::Less),
        (Some(a), Some(b)) => Some(moment(b).cmp(&moment(a))).filter(|order| order.is_ne()),
    }
}

/// The order `sort` puts two things in, as the core's own sorts do it.
fn order(sort: LibrarySort, a: &LibraryItem, b: &LibraryItem) -> Ordering {
    let by_title = || a.title.to_lowercase().cmp(&b.title.to_lowercase());
    match sort {
        LibrarySort::Alphabetical => by_title(),
        LibrarySort::Creator => a
            .subtitle
            .to_lowercase()
            .cmp(&b.subtitle.to_lowercase())
            .then_with(by_title),
        LibrarySort::Added => {
            newest_first(a.added_at.as_ref(), b.added_at.as_ref()).unwrap_or_else(by_title)
        }
        LibrarySort::Recent => newest_first(a.last_played_at.as_ref(), b.last_played_at.as_ref())
            .unwrap_or_else(by_title),
    }
}

/// The lines to show. Looking for something (a chip is on, or something is
/// typed) lists every match flat; otherwise folders hold what was put in
/// them and show it when open.
pub fn arranged<'a>(items: &'a [LibraryItem], folders: &'a [Folder], view: &View) -> Vec<Row<'a>> {
    let query = view.query.trim().to_lowercase();
    // What is typed is looked for in the title, in whose it is, and in the
    // name of the folder it is filed in.
    let matches = |item: &LibraryItem| {
        let folder = folders.iter().find(|folder| folder.id == item.folder_id);
        query.is_empty()
            || item.title.to_lowercase().contains(&query)
            || item.subtitle.to_lowercase().contains(&query)
            || folder.is_some_and(|folder| folder.name.to_lowercase().contains(&query))
    };
    let mut matching: Vec<&LibraryItem> = items
        .iter()
        .filter(|item| view.kind.is_none_or(|kind| item.kind == kind))
        .filter(|item| matches(item))
        .collect();
    // Pinned things come first under every order.
    matching.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| order(view.sort, a, b)));

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

    fn sorted(library: &[LibraryItem], sort: LibrarySort) -> Vec<String> {
        let none = HashSet::new();
        titles(&arranged(library, &[], &view(None, "", sort, &none)))
    }

    #[test]
    fn the_library_is_narrowed_by_kind_and_by_what_is_typed() {
        let library = [
            item("Road trip", LibraryKind::Playlist, false),
            item("Discovery", LibraryKind::Album, false),
            item("Roads", LibraryKind::Album, false),
        ];
        let none = HashSet::new();
        let albums = view(
            Some(LibraryKind::Album),
            "",
            LibrarySort::Alphabetical,
            &none,
        );
        assert_eq!(
            titles(&arranged(&library, &[], &albums)),
            ["Discovery", "Roads"]
        );
        let typed = view(None, " ROAD ", LibrarySort::Alphabetical, &none);
        assert_eq!(
            titles(&arranged(&library, &[], &typed)),
            ["Road trip", "Roads"]
        );
    }

    #[test]
    fn what_is_typed_is_also_looked_for_in_whose_it_is() {
        let library = [
            LibraryItem {
                subtitle: "Daft Punk".into(),
                ..item("Discovery", LibraryKind::Album, false)
            },
            item("Road trip", LibraryKind::Playlist, false),
        ];
        let none = HashSet::new();
        let typed = view(None, "daft", LibrarySort::Recent, &none);
        assert_eq!(titles(&arranged(&library, &[], &typed)), ["Discovery"]);
    }

    #[test]
    fn pinned_items_come_first_in_every_order() {
        let library = [
            item("Zebra", LibraryKind::Playlist, false),
            item("Liked Music", LibraryKind::Playlist, true),
            item("apple", LibraryKind::Playlist, false),
        ];
        for sort in LibrarySort::EVERY {
            assert_eq!(
                sorted(&library, sort),
                ["Liked Music", "apple", "Zebra"],
                "{sort:?}"
            );
        }
    }

    #[test]
    fn recents_puts_what_was_played_last_first_and_the_unplayed_after() {
        let played = |title: &str, at: &str| LibraryItem {
            last_played_at: Some(at.into()),
            ..item(title, LibraryKind::Album, false)
        };
        let library = [
            item("Never played", LibraryKind::Album, false),
            played("Yesterday", "2026-10-03T09:00:00Z"),
            played("Just now", "2026-10-04T09:00:00.25Z"),
            played("A moment before", "2026-10-04T09:00:00Z"),
            item("Also never", LibraryKind::Album, false),
        ];
        assert_eq!(
            sorted(&library, LibrarySort::Recent),
            [
                "Just now",
                "A moment before",
                "Yesterday",
                "Also never",
                "Never played"
            ]
        );
    }

    #[test]
    fn recently_added_puts_the_newest_first() {
        let added = |title: &str, at: &str| LibraryItem {
            added_at: Some(at.into()),
            ..item(title, LibraryKind::Album, false)
        };
        let library = [
            added("Old", "2025-01-01T00:00:00Z"),
            item("Before the count", LibraryKind::Album, false),
            added("New", "2026-06-01T00:00:00Z"),
        ];
        assert_eq!(
            sorted(&library, LibrarySort::Added),
            ["New", "Old", "Before the count"]
        );
    }

    #[test]
    fn creator_sorts_by_whose_it_is_then_by_title() {
        let by = |title: &str, creator: &str| LibraryItem {
            subtitle: creator.into(),
            ..item(title, LibraryKind::Album, false)
        };
        let library = [
            by("Homework", "Daft Punk"),
            by("Blue", "Joni Mitchell"),
            by("Discovery", "daft punk"),
        ];
        assert_eq!(
            sorted(&library, LibrarySort::Creator),
            ["Discovery", "Homework", "Blue"]
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
        let closed = view(None, "", LibrarySort::Alphabetical, &shut);
        assert_eq!(
            titles(&arranged(&library, &folders, &closed)),
            ["Liked Music", "[Sport]", "Lost", "Road trip"]
        );
        let opened = HashSet::from(["f1".to_owned()]);
        let open = view(None, "", LibrarySort::Alphabetical, &opened);
        assert_eq!(
            titles(&arranged(&library, &folders, &open)),
            ["Liked Music", "[Sport]", "  Running", "Lost", "Road trip"]
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
        // The folder's own name finds what is in it, too.
        let by_folder = view(None, "sport", LibrarySort::Recent, &shut);
        assert_eq!(
            titles(&arranged(&library, &folders, &by_folder)),
            ["Running"]
        );
    }
}
