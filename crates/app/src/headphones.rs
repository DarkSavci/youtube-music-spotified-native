//! Headphone correction from AutoEQ.
//!
//! No headphone is neutral: most are several decibels off it, a lift in the
//! bass here, a peak at 3 kHz there, which no change to the stream comes
//! near. AutoEQ (Jaakko Pasanen's project, MIT licensed) publishes, for
//! some nine thousand headphones measured by reviewers such as oratory1990
//! and crinacle, the ten filters that bring each to a neutral target. Here
//! the list is fetched and kept, the listener picks theirs, and its filters
//! are kept in the settings, so the correction needs no network again.
//!
//! The filters run in the engine's equalizer, ahead of the ten sliders,
//! whether the sliders are on or not; the level is taken down by the
//! correction's highest point, so it never clips.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use spotified_audio::eq;

/// Where AutoEQ's results are, as files.
const RESULTS: &str = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/";
/// The list of every headphone it has a profile for.
const INDEX: &str = "INDEX.md";
/// The list kept on disk, and how long it is kept before it is asked for
/// again: new headphones are added every week or so.
const INDEX_FILE: &str = "autoeq-index.md";
const INDEX_FRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// The most matches a search shows.
pub const MATCHES: usize = 8;
/// The reviewer whose measurements are put first where a headphone has
/// several: his are the ones AutoEQ itself recommends.
const PREFERRED_SOURCE: &str = "oratory1990";

/// Looking for a headphone, on the Settings page.
#[derive(Debug, Default)]
pub struct Search {
    /// What has been typed.
    pub query: String,
    /// AutoEQ's list, once it has been fetched or read.
    pub list: Option<Result<std::sync::Arc<Vec<Headphone>>, String>>,
    /// Being fetched now.
    pub listing: bool,
    /// The headphone whose correction is being fetched now.
    pub fetching: Option<String>,
}

/// A headphone AutoEQ has a profile for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headphone {
    pub name: String,
    /// Who measured it.
    pub source: String,
    /// Its folder under the results, as the index writes it: escaped.
    path: String,
}

/// A headphone's correction as kept in the settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Correction {
    pub name: String,
    pub source: String,
    pub filters: Vec<Filter>,
}

/// One filter of a correction, as AutoEQ writes it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub kind: Kind,
    pub hz: f32,
    pub db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Peak,
    LowShelf,
    HighShelf,
}

impl Correction {
    /// The filters as the engine's equalizer takes them.
    pub fn for_engine(&self) -> eq::Correction {
        let mut correction = eq::Correction::default();
        for (slot, filter) in correction.filters.iter_mut().zip(&self.filters) {
            *slot = eq::Filter {
                kind: match filter.kind {
                    Kind::Peak => eq::FilterKind::Peak,
                    Kind::LowShelf => eq::FilterKind::LowShelf,
                    Kind::HighShelf => eq::FilterKind::HighShelf,
                },
                hz: filter.hz,
                db: filter.db,
                q: filter.q,
            };
        }
        correction
    }
}

/// The headphones in AutoEQ's index: lines such as
/// `- [Sennheiser HD 600](./oratory1990/over-ear/Sennheiser%20HD%20600) by oratory1990`.
pub fn parse_index(text: &str) -> Vec<Headphone> {
    text.lines()
        .filter_map(|line| {
            let line = line.strip_prefix("- [")?;
            let (name, rest) = line.split_once("](./")?;
            // The path ends at ") by ", not at the first ")": names such
            // as "1MORE Aero (ANC Off)" carry brackets of their own.
            let (path, source) = rest.split_once(") by ")?;
            let source = source.split(" on ").next().unwrap_or(source).trim();
            Some(Headphone {
                name: name.to_owned(),
                source: source.to_owned(),
                path: path.to_owned(),
            })
        })
        .collect()
}

/// The headphones whose names hold every word of `query`, best first:
/// those whose name starts with it, then the preferred reviewer's, then the
/// shorter names, which are the plainer models.
pub fn search<'a>(headphones: &'a [Headphone], query: &str) -> Vec<&'a Headphone> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<&Headphone> = headphones
        .iter()
        .filter(|headphone| {
            let name = headphone.name.to_lowercase();
            words.iter().all(|word| name.contains(word))
        })
        .collect();
    found.sort_by_key(|headphone| {
        let name = headphone.name.to_lowercase();
        (
            !name.starts_with(&query),
            headphone.source != PREFERRED_SOURCE,
            headphone.name.len(),
        )
    });
    found.truncate(MATCHES);
    found
}

/// A profile's filters, from AutoEQ's `ParametricEQ.txt`:
/// `Filter 1: ON LSC Fc 105 Hz Gain 6.5 dB Q 0.70`. Its preamp line is
/// left: the engine works out its own from the curve. `None` if no filter
/// could be read.
pub fn parse_parametric(text: &str) -> Option<Vec<Filter>> {
    let filters: Vec<Filter> = text
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once(':')?;
            let words: Vec<&str> = rest.split_whitespace().collect();
            if words.first() != Some(&"ON") {
                return None;
            }
            let kind = match *words.get(1)? {
                "PK" | "PEQ" => Kind::Peak,
                "LSC" | "LS" | "LSQ" => Kind::LowShelf,
                "HSC" | "HS" | "HSQ" => Kind::HighShelf,
                _ => return None,
            };
            let after = |key: &str| -> Option<f32> {
                let at = words.iter().position(|word| *word == key)?;
                words.get(at + 1)?.parse().ok()
            };
            Some(Filter {
                kind,
                hz: after("Fc")?,
                db: after("Gain")?,
                q: after("Q")?,
            })
        })
        .take(eq::CORRECTION_FILTERS)
        .collect();
    (!filters.is_empty()).then_some(filters)
}

/// AutoEQ's list, from the copy kept in `cache_dir` while it is fresh, and
/// otherwise from GitHub. Offline, a stale copy is better than none.
pub fn load_index(agent: &ureq::Agent, cache_dir: &Path) -> Result<Vec<Headphone>, String> {
    let kept = cache_dir.join(INDEX_FILE);
    let age = std::fs::metadata(&kept)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok());
    if age.is_some_and(|age| age < INDEX_FRESH)
        && let Ok(text) = std::fs::read_to_string(&kept)
    {
        return Ok(parse_index(&text));
    }
    match crate::update::fetch_text(agent, &format!("{RESULTS}{INDEX}")) {
        Ok(Some(text)) => {
            if let Err(error) =
                std::fs::create_dir_all(cache_dir).and_then(|()| std::fs::write(&kept, &text))
            {
                log::debug!("the headphone list was not kept: {error}");
            }
            Ok(parse_index(&text))
        }
        fetched => match std::fs::read_to_string(&kept) {
            Ok(text) => Ok(parse_index(&text)),
            Err(_) => Err(match fetched {
                Err(error) => format!("The headphone list could not be fetched: {error}"),
                _ => "The headphone list was not there.".to_owned(),
            }),
        },
    }
}

/// The correction for `headphone`.
pub fn fetch(agent: &ureq::Agent, headphone: &Headphone) -> Result<Correction, String> {
    // The folder is named after the headphone, and so is the file in it.
    let folder = headphone.path.trim_end_matches('/');
    let named = folder.rsplit('/').next().unwrap_or(folder);
    let url = format!("{RESULTS}{folder}/{named}%20ParametricEQ.txt");
    let text = crate::update::fetch_text(agent, &url)
        .map_err(|error| format!("The correction could not be fetched: {error}"))?
        .ok_or("AutoEQ has no parametric correction for this headphone.")?;
    let filters = parse_parametric(&text).ok_or("The correction could not be read.")?;
    Ok(Correction {
        name: headphone.name.clone(),
        source: headphone.source.clone(),
        filters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX_TEXT: &str = "# Index\n\
        - [Sennheiser HD 600](./oratory1990/over-ear/Sennheiser%20HD%20600) by oratory1990\n\
        - [Sennheiser HD 600](./crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%20600) by crinacle on GRAS 43AG-7\n\
        - [Sennheiser HD 6XX](./crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%206XX) by crinacle on GRAS 43AG-7\n\
        - [Apple AirPods Pro](./oratory1990/in-ear/Apple%20AirPods%20Pro) by oratory1990\n\
        - [1MORE Aero (ANC Off)](./HypetheSonics/GRAS%20RA0045%20in-ear/1MORE%20Aero%20(ANC%20Off)) by HypetheSonics on GRAS RA0045\n";

    #[test]
    fn the_index_is_read_with_its_reviewers() {
        let headphones = parse_index(INDEX_TEXT);
        assert_eq!(headphones.len(), 5);
        assert_eq!(headphones[4].name, "1MORE Aero (ANC Off)");
        assert_eq!(headphones[4].source, "HypetheSonics");
        assert!(headphones[4].path.ends_with("1MORE%20Aero%20(ANC%20Off)"));
        assert_eq!(headphones[1].name, "Sennheiser HD 600");
        assert_eq!(headphones[1].source, "crinacle");
        assert_eq!(
            headphones[0].path,
            "oratory1990/over-ear/Sennheiser%20HD%20600"
        );
    }

    #[test]
    fn a_search_puts_the_closest_and_the_preferred_reviewer_first() {
        let headphones = parse_index(INDEX_TEXT);
        let found = search(&headphones, "hd 6");
        assert_eq!(found.len(), 3);
        let found = search(&headphones, "sennheiser hd 600");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].source, "oratory1990");
        assert!(search(&headphones, "  ").is_empty());
        assert_eq!(search(&headphones, "airpods")[0].name, "Apple AirPods Pro");
    }

    #[test]
    fn a_parametric_profile_is_read_filter_by_filter() {
        let text = "Preamp: -6.3 dB\n\
            Filter 1: ON LSC Fc 105 Hz Gain 6.5 dB Q 0.70\n\
            Filter 2: ON PK Fc 125 Hz Gain -2.7 dB Q 0.55\n\
            Filter 3: OFF PK Fc 8445 Hz Gain 3.3 dB Q 1.61\n\
            Filter 4: ON HSC Fc 10000 Hz Gain -3.1 dB Q 0.70\n";
        let filters = parse_parametric(text).unwrap_or_default();
        assert_eq!(filters.len(), 3);
        assert_eq!(
            filters[0],
            Filter {
                kind: Kind::LowShelf,
                hz: 105.0,
                db: 6.5,
                q: 0.7
            }
        );
        assert_eq!(filters[2].kind, Kind::HighShelf);
        assert_eq!(parse_parametric("Preamp: -6 dB\n"), None);
        let engine = Correction {
            name: String::new(),
            source: String::new(),
            filters,
        }
        .for_engine();
        assert_eq!(engine.filters[1].kind, eq::FilterKind::Peak);
        assert_eq!(engine.filters[3].kind, eq::FilterKind::Off);
    }

    /// The real thing, over the network: AutoEQ's list as it is today, a
    /// search in it, and a correction fetched from where the list says.
    /// `cargo test -p spotified headphones -- --ignored`.
    #[test]
    #[ignore = "fetches from GitHub"]
    fn autoeq_is_fetched_and_read_as_published() -> Result<(), String> {
        let agent = crate::update::agent();
        let cache = std::env::temp_dir().join(format!("autoeq-test-{}", std::process::id()));
        let headphones = load_index(&agent, &cache)?;
        assert!(headphones.len() > 5000, "{} headphones", headphones.len());
        // Kept, and read back without the network.
        assert_eq!(load_index(&agent, &cache)?.len(), headphones.len());
        let _ = std::fs::remove_dir_all(&cache);
        let found = search(&headphones, "sennheiser hd 600");
        let first = found.first().ok_or("no HD 600")?;
        assert_eq!(first.source, PREFERRED_SOURCE);
        let correction = fetch(&agent, first)?;
        assert_eq!(correction.filters.len(), 10);
        assert_eq!(correction.filters[0].kind, Kind::LowShelf);
        // A name with brackets of its own, whose folder has them too.
        let bracketed = search(&headphones, "1MORE Aero (ANC Off)");
        let aero = bracketed.first().ok_or("no 1MORE Aero")?;
        assert!(!fetch(&agent, aero)?.filters.is_empty());
        Ok(())
    }
}
