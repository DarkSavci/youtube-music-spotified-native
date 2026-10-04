//! What changed in each release, read from the `CHANGELOG.md` the build
//! was made with. Nothing is fetched: the notes travel with the app.

use std::sync::OnceLock;

/// The version this build is.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// As written in the file: `2026-10-04`. Empty when it gives none.
    pub date: String,
    pub groups: Vec<Group>,
    /// A line said of the whole release, when it lists nothing.
    pub note: String,
}

/// The changes of one kind: "New", "Fixed", "Faster".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Group {
    pub title: String,
    pub items: Vec<String>,
}

/// Every release, newest first.
pub fn releases() -> &'static [Release] {
    static RELEASES: OnceLock<Vec<Release>> = OnceLock::new();
    RELEASES.get_or_init(|| parse(include_str!("../../../CHANGELOG.md")))
}

/// The newest release the notes know of; empty if they know of none.
pub fn latest() -> &'static str {
    releases().first().map_or("", |release| &release.version)
}

/// Reads the file: `## version — date` starts a release, `### title` a
/// group within it, and `- text` a change within that.
fn parse(text: &str) -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(heading) = line.strip_prefix("## ") {
            let (version, date) = heading.split_once(" — ").unwrap_or((heading, ""));
            releases.push(Release {
                version: version.trim().to_owned(),
                date: date.trim().to_owned(),
                ..Release::default()
            });
            continue;
        }
        // Whatever comes before the first release is the file's own heading.
        let Some(release) = releases.last_mut() else {
            continue;
        };
        if let Some(title) = line.strip_prefix("### ") {
            release.groups.push(Group {
                title: title.to_owned(),
                items: Vec::new(),
            });
        } else if let Some(item) = line.strip_prefix("- ") {
            if let Some(group) = release.groups.last_mut() {
                group.items.push(item.to_owned());
            }
        } else if !line.is_empty() && release.groups.is_empty() {
            release.note = line.to_owned();
        }
    }
    releases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_has_its_date_and_its_changes_by_kind() {
        let releases = parse(
            "# Changelog\n\nEvery release.\n\n## 0.2.0 — 2026-10-04\n\n### New\n\n- One\n- Two\n\n\
             ### Fixed\n\n- Three\n\n## 0.1.0\n\nBehind-the-scenes improvements only.\n",
        );
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].version, "0.2.0");
        assert_eq!(releases[0].date, "2026-10-04");
        assert_eq!(releases[0].groups[0].title, "New");
        assert_eq!(releases[0].groups[0].items, ["One", "Two"]);
        assert_eq!(releases[0].groups[1].items, ["Three"]);
        assert_eq!(releases[1].date, "");
        assert_eq!(releases[1].note, "Behind-the-scenes improvements only.");
    }

    #[test]
    fn the_notes_shipped_with_the_build_begin_with_the_build_itself() {
        assert_eq!(latest(), VERSION);
        assert!(releases().iter().all(|release| !release.groups.is_empty()));
    }
}
