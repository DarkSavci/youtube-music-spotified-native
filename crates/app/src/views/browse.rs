//! The pages YouTube Music lays out itself: explore, the moods and genres,
//! one mood's page, the whole of a shelf. And what the account played lately.

use eframe::egui::{self, Color32, Sense, Ui, vec2};
use spotified_client::models::{BrowsePage, MoodChip, Shelf, Track};

use super::{cards, pages, tracks, widgets};
use crate::actions::Action;
use crate::state::{Page, State, Surface};
use crate::theme::{self, Icon};

/// The narrowest a tile gets; a row holds as many as fit at this width.
const TILE_MIN_WIDTH: f32 = 180.0;
const TILE_GAP: f32 = 16.0;
const TILE_PADDING: f32 = 16.0;
/// How many tiles may ask for their picture on one frame.
const TILE_ART_PER_FRAME: usize = 2;

pub fn surface(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, surface: &Surface) {
    let skeleton = pages::Skeleton::Tracks(4);
    let page = state.surfaces.get(&surface.key());
    pages::loaded_or(state, ui, page, skeleton, |ui, page| {
        let title = if page.title.is_empty() {
            &surface.title
        } else {
            &page.title
        };
        ui.add_space(8.0);
        pages::title(ui, title, 24.0);
        let shown: Vec<&Shelf> = shown_shelves(page).collect();
        if page.moods.is_empty() && shown.is_empty() {
            let text = "YouTube Music sent nothing this app can show here.";
            widgets::empty_state(ui, &state.palette, Icon::LayoutGrid, "Nothing here", text);
            return;
        }
        // Tiles without a colour are not moods but ways onward: new
        // releases, the charts. They go first, as chips.
        let (ways, moods): (Vec<&MoodChip>, Vec<&MoodChip>) =
            page.moods.iter().partition(|mood| mood.color.is_empty());
        if !ways.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for way in ways {
                    if widgets::chip(ui, &state.palette, &way.title, false).clicked() {
                        actions.push(Action::Open(Page::Browse(Surface {
                            id: way.id.clone(),
                            params: way.params.clone(),
                            title: way.title.clone(),
                        })));
                    }
                }
            });
        }
        // A page that is one shelf is the whole of something: a grid reads
        // better than a single row to scroll.
        if let [only] = shown[..] {
            // Named after the page it is on, it would only say so twice.
            if !same_title(&only.title, title) {
                cards::section_title(ui, &only.title);
            }
            cards::grid(ui, only.items.len(), |ui, index| {
                cards::item(state, ui, actions, &only.items[index]);
            });
        } else {
            // What there is to hear, with its covers, before the tiles.
            shelves(state, ui, actions, &page.shelves, "surface");
        }
        if !moods.is_empty() {
            if !shown.is_empty() {
                cards::section_title(ui, "Moods and genres");
                ui.add_space(6.0);
            }
            mood_tiles(state, ui, actions, &moods);
        }
    });
}

/// Whether two titles are the same once case, and the room YouTube pads
/// them with, are set aside.
fn same_title(a: &str, b: &str) -> bool {
    !a.trim().is_empty() && a.trim().eq_ignore_ascii_case(b.trim())
}

/// A shelf with nothing this client can show is left out.
fn shown_shelves(page: &BrowsePage) -> impl Iterator<Item = &Shelf> {
    page.shelves.iter().filter(|shelf| !shelf.items.is_empty())
}

/// Shelves, each a row of as many cards as fit, with a way to the whole
/// of it where YouTube Music has more. `group` tells these shelves from
/// any others on the page: two may share a title.
pub fn shelves(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    shelves: &[Shelf],
    group: &str,
) {
    let shown = shelves.iter().filter(|shelf| !shelf.items.is_empty());
    for (index, shelf) in shown.enumerate() {
        let whole = (!shelf.show_all_id.is_empty()).then(|| {
            Page::Browse(Surface {
                id: shelf.show_all_id.clone(),
                params: shelf.show_all_params.clone(),
                title: shelf.title.clone(),
            })
        });
        ui.push_id((group, index), |ui| {
            // A row of cards with nothing said over it still stands apart.
            if shelf.title.is_empty() {
                ui.add_space(16.0);
            } else {
                cards::linked_title(state, ui, actions, (&shelf.title, "Show all"), whole);
            }
            cards::row(ui, shelf.items.len(), |ui, index| {
                cards::item(state, ui, actions, &shelf.items[index]);
            });
        });
    }
}

/// Moods and genres as Spotify shows its categories: a wall of coloured
/// tiles, each in the colour YouTube Music gives it, with a cover from its
/// page tipped into the corner.
pub fn mood_tiles(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, moods: &[&MoodChip]) {
    if moods.is_empty() {
        return;
    }
    // As many columns as fit at the least width, sharing the row evenly.
    let room = ui.available_width();
    let columns = ((room + TILE_GAP) / (TILE_MIN_WIDTH + TILE_GAP))
        .floor()
        .max(1.0);
    let width = ((room - TILE_GAP * (columns - 1.0)) / columns).floor();
    let size = vec2(width, (width * 9.0 / 16.0).round());
    // The pictures are asked for a few at a time, as tiles come into view.
    let mut asked = 0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(TILE_GAP, TILE_GAP);
        for mood in moods {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            widgets::name(ui, &response, &mood.title);
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let key = (mood.id.clone(), mood.params.clone());
            let art = state.tile_art.get(&key);
            if art.is_none() && asked < TILE_ART_PER_FRAME {
                asked += 1;
                actions.push(Action::WantTileArt(key.0, key.1));
            }
            let art = art.and_then(Option::as_deref).unwrap_or_default();
            tile(state, ui, &response, rect, mood, art);
            if response.clicked() {
                actions.push(Action::Open(Page::Browse(Surface {
                    id: mood.id.clone(),
                    params: mood.params.clone(),
                    title: mood.title.clone(),
                })));
            }
        }
    });
}

/// One tile: its colour, its name at the top left, and its picture tipped
/// over at the bottom right, running off the edge.
fn tile(
    state: &State,
    ui: &mut Ui,
    response: &egui::Response,
    rect: egui::Rect,
    mood: &MoodChip,
    art: &[spotified_client::models::Artwork],
) {
    let fill = hex_color(&mood.color).unwrap_or(state.palette.surface_active);
    let lift = widgets::hover(ui, response);
    let fill = crate::tint::blend(fill, Color32::WHITE, 0.1 * lift);
    ui.painter().rect_filled(rect, theme::RADIUS, fill);
    let side = rect.width() * 0.46;
    let picture = egui::Rect::from_min_size(
        egui::pos2(rect.right() - side * 0.89, rect.bottom() - side * 0.74),
        vec2(side, side),
    );
    widgets::artwork_tipped(ui, state, art, picture, rect);
    let width = rect.width() * 0.7 - TILE_PADDING;
    let font = theme::bold(20.0);
    let galley = widgets::elided(ui, &mood.title, font, Color32::WHITE, width, 2);
    let at = rect.left_top() + vec2(TILE_PADDING, TILE_PADDING);
    // A soft dark copy under the name keeps it readable over a cover.
    ui.painter().galley(
        at + vec2(0.0, 1.0),
        galley.clone(),
        Color32::from_black_alpha(90),
    );
    ui.painter().galley(at, galley, Color32::WHITE);
}

/// The ways onward that head Browse all, ahead of the moods and genres:
/// the app's own tiles, in colours of its choosing.
pub fn first_tiles() -> [MoodChip; 3] {
    let tile = |id: &str, title: &str, color: &str| MoodChip {
        id: id.to_owned(),
        params: String::new(),
        title: title.to_owned(),
        color: color.to_owned(),
    };
    [
        tile("FEmusic_explore", "Discover", "#185a74"),
        tile("FEmusic_charts", "Charts", "#78468c"),
        tile("FEmusic_new_releases", "New releases", "#315e46"),
    ]
}

/// `#RRGGBB` as a colour, darkened a little so white text reads on it.
fn hex_color(text: &str) -> Option<Color32> {
    let digits = text.strip_prefix('#').filter(|digits| digits.len() == 6)?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    let [_, red, green, blue] = value.to_be_bytes();
    Some(crate::tint::blend(
        Color32::from_rgb(red, green, blue),
        Color32::BLACK,
        0.28,
    ))
}

/// What the account played lately, on any device.
pub fn history(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.add_space(8.0);
    pages::title(ui, "Recently played", 24.0);
    pages::loaded(state, ui, &state.history, |ui, played| {
        if played.is_empty() {
            let text = "Songs you play show up here.";
            widgets::empty_state(ui, &state.palette, Icon::Clock, "Nothing played yet", text);
            return;
        }
        let songs: Vec<&Track> = played.iter().collect();
        let list = tracks::List {
            tracks: &songs,
            origin: "Recently played",
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: true,
            },
            mode: tracks::Mode::List,
        };
        tracks::table(state, ui, actions, list);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_colour_is_read_from_its_hex() {
        assert_eq!(hex_color("#000000"), Some(Color32::from_rgb(0, 0, 0)));
        assert!(hex_color("#FF8000").is_some());
        assert_eq!(hex_color("FF8000"), None);
        assert_eq!(hex_color("#FFF"), None);
        assert_eq!(hex_color(""), None);
    }
}
