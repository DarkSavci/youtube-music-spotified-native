//! The words about what a page shows: an artist's biography, what is
//! said of an album, a show's notes.

use eframe::egui::{self, Sense, Ui};

use crate::actions::Action;
use crate::state::State;
use crate::theme;
use crate::views::{cards, widgets};

/// Such text is read as a column, not across the window.
const WIDTH: f32 = 680.0;
/// Longer than this, and it is clipped until asked for in full: these run
/// to well over a thousand characters, and a wall of prose between the
/// music and the rest of the page buries both.
const LONG: usize = 400;
/// How many lines of a clipped text are shown.
const CLIPPED_LINES: usize = 5;

/// A paragraph, held to a column; clipped to a few lines if `clipped`.
pub fn text(state: &State, ui: &mut Ui, text: &str, clipped: bool) {
    let palette = &state.palette;
    let width = WIDTH.min(ui.available_width());
    let font = theme::regular(14.0);
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, palette.secondary, width);
    // Room between the lines: this is prose to be read.
    for section in &mut job.sections {
        section.format.line_height = Some(23.0);
    }
    if clipped {
        job.wrap.max_rows = CLIPPED_LINES;
    }
    let galley = ui.painter().layout_job(job);
    // Prose that may be quoted: a drag across it selects it.
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::drag());
    if ui.is_rect_visible(rect) {
        ui.scope(|ui| {
            widgets::selectable(ui);
            widgets::selectable_galley(ui, &response, rect.min, galley, palette.secondary);
        });
    }
}

/// The "About" section. `source` is where the text came from, when it
/// names one: it is not ours, and prose with no provenance is worth less.
pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, about: &str, source: &str) {
    let long = about.chars().count() > LONG;
    let expanded = state.about_expanded;
    let link = long.then_some(if expanded { "Show less" } else { "Show all" });
    if cards::title_with(state, ui, "About", link) {
        actions.push(Action::ToggleAbout);
    }
    text(state, ui, about, long && !expanded);
    let Some(host) = host(source) else {
        return;
    };
    ui.add_space(12.0);
    let palette = &state.palette;
    let font = theme::regular(12.0);
    let label = ui
        .painter()
        .layout_no_wrap("Source: ".to_owned(), font.clone(), palette.dim);
    let (rect, _) = ui.allocate_exact_size(label.size(), Sense::hover());
    ui.painter().galley(rect.min, label, palette.dim);
    let link = widgets::Link {
        text: host,
        font,
        color: palette.secondary,
        width: 320.0,
    };
    if link.show(ui, ui.id().with("about-source"), rect.right_top(), true) {
        ui.ctx().open_url(egui::OpenUrl::new_tab(source));
    }
}

/// The site an address is on, as it is said aloud: "en.wikipedia.org".
fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_is_named_by_its_site() {
        assert_eq!(
            host("https://en.wikipedia.org/wiki/Daft_Punk"),
            Some("en.wikipedia.org")
        );
        assert_eq!(host("https://www.example.com"), Some("example.com"));
        assert_eq!(host("mailto:someone"), None);
        assert_eq!(host(""), None);
    }
}
