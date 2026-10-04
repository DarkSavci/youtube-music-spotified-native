//! Text placed by hand: at a point, cut to fit, set tight, or as a link.

use std::sync::Arc;

use eframe::egui::text::CCursor;
use eframe::egui::text_selection::LabelSelectionState;
use eframe::egui::{self, Align2, Color32, Galley, Rect, Response, Sense, Ui};
use spotified_client::models::ArtistRef;

/// Text painted at a point, for layouts that place things by hand.
pub fn text_at(
    ui: &Ui,
    pos: egui::Pos2,
    anchor: Align2,
    text: &str,
    font: egui::FontId,
    color: Color32,
) -> Rect {
    ui.painter().text(pos, anchor, text, font, color)
}

/// Lays text out to fit `width`, ending in an ellipsis after `rows` lines.
pub fn elided(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    width: f32,
    rows: usize,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, width);
    job.wrap.max_rows = rows;
    // A single line is cut where it runs out; several lines break at words.
    job.wrap.break_anywhere = rows == 1;
    ui.painter().layout_job(job)
}

/// As [`elided`], with the letters `tracking` points closer together
/// (negative) or further apart: a large title is set tight, a small label
/// in capitals loose.
pub fn tracked(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    tracking: f32,
    (width, rows): (f32, usize),
) -> Arc<Galley> {
    let format = egui::TextFormat {
        font_id: font,
        color,
        extra_letter_spacing: tracking,
        ..egui::TextFormat::default()
    };
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), format);
    job.wrap.max_width = width;
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = rows == 1;
    ui.painter().layout_job(job)
}

/// Lets the labels drawn in `ui` from here on be selected with the pointer
/// and copied with Ctrl+C. Text in a music client is mostly rows to click,
/// which is why it is off elsewhere; prose that someone may want to quote
/// (release notes, a biography, an error) asks for it.
pub fn selectable(ui: &mut Ui) {
    ui.style_mut().interaction.selectable_labels = true;
    // Selected text keeps its own colour on the highlight; in the accent,
    // as egui would have it, it is red on red.
    let ink = ui.visuals().text_color();
    ui.visuals_mut().selection.stroke.color = ink;
}

/// Paints a galley laid out by hand so that it can be selected and copied
/// as a label can. `response` is the text's own, sensing a drag: the drag
/// is what selects, and a click on it still counts as a click.
pub fn selectable_galley(
    ui: &Ui,
    response: &egui::Response,
    at: egui::Pos2,
    galley: Arc<Galley>,
    color: Color32,
) {
    // The caret says it is text, unless it is also something to click.
    if response.hovered() && !response.sense.senses_click() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    let none = egui::Stroke::NONE;
    LabelSelectionState::label_text_selection(ui, response, at, galley, color, none);
}

/// What goes between two artists' names.
const BETWEEN_ARTISTS: &str = ", ";

/// A song's artists on one line, each name a link to that artist's own
/// page: a song by two is as much the second's as the first's. An artist
/// YouTube has no page for is plain text among them, as in the Electron
/// app (`EntityLinks.tsx`).
pub struct Artists<'a> {
    pub artists: &'a [ArtistRef],
    pub font: egui::FontId,
    pub color: Color32,
    /// The room the line has; longer ends in an ellipsis.
    pub width: f32,
}

impl Artists<'_> {
    /// Paints the line with its top left at `at`. Returns the id of the
    /// artist whose name was clicked, and how wide the line came out.
    pub fn show(&self, ui: &Ui, id: egui::Id, at: egui::Pos2) -> (Option<String>, f32) {
        let mut clicked = None;
        let width = self.show_each(ui, id, at, |artist, response| {
            if response.clicked() {
                clicked = Some(artist_page_id(&artist.id).to_owned());
            }
        });
        (clicked, width)
    }

    /// As [`Artists::show`], handing each name that leads somewhere, and
    /// what the pointer did with it, to `each`: for a caller that hangs a
    /// menu on a name as well. Returns how wide the line came out.
    pub fn show_each(
        &self,
        ui: &Ui,
        id: egui::Id,
        at: egui::Pos2,
        mut each: impl FnMut(&ArtistRef, &Response),
    ) -> f32 {
        let named = || self.artists.iter().filter(|artist| !artist.name.is_empty());
        let line = named()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(BETWEEN_ARTISTS);
        let galley = elided(ui, &line, self.font.clone(), self.color, self.width, 1);
        let size = galley.size();
        // Where each name lies, counted in characters as the galley counts.
        let mut start = 0;
        for (index, artist) in named().enumerate() {
            let end = start + artist.name.chars().count();
            let edge = |at: usize| galley.pos_from_cursor(CCursor::new(at)).left();
            let (left, right) = (edge(start), edge(end).min(size.x));
            start = end + BETWEEN_ARTISTS.chars().count();
            // No page, or cut off by the ellipsis: nothing to click.
            if artist.id.is_empty() || right - left < 1.0 {
                continue;
            }
            let name = Rect::from_min_max(
                egui::pos2(at.x + left, at.y),
                egui::pos2(at.x + right, at.y + size.y),
            );
            let response = ui.interact(name, id.with(index), Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter()
                    .hline(name.x_range(), name.bottom() - 1.0, (1.0, self.color));
            }
            each(artist, &response);
        }
        ui.painter().galley(at, galley, self.color);
        size.x
    }
}

/// The id an artist's page goes by. One from the library arrives wrapped
/// ("MPLA" before the channel's id); the page wants the channel.
pub fn artist_page_id(id: &str) -> &str {
    match id.strip_prefix("MPLA") {
        Some(channel) if channel.starts_with("UC") => channel,
        _ => id,
    }
}

/// A line of text that leads somewhere: underlined, with the hand cursor,
/// while the pointer is on it.
pub struct Link<'a> {
    pub text: &'a str,
    pub font: egui::FontId,
    pub color: Color32,
    /// The room it has; longer text ends in an ellipsis.
    pub width: f32,
}

impl Link<'_> {
    /// Paints the text with its top left at `at` and returns whether it
    /// was clicked. With `leads_somewhere` false it is plain text.
    pub fn show(&self, ui: &Ui, id: egui::Id, at: egui::Pos2, leads_somewhere: bool) -> bool {
        self.show_measured(ui, id, at, leads_somewhere).0
    }

    /// As [`Link::show`], and how wide the text came out, for a caller
    /// that places something after it.
    pub fn show_measured(
        &self,
        ui: &Ui,
        id: egui::Id,
        at: egui::Pos2,
        leads_somewhere: bool,
    ) -> (bool, f32) {
        let (response, width) = self.place(ui, id, at, leads_somewhere, leads_somewhere);
        (response.is_some_and(|response| response.clicked()), width)
    }

    /// As [`Link::show_measured`], for a caller that hangs a menu on the
    /// text: it answers the pointer whether or not it leads somewhere, and
    /// what the pointer did with it is handed back.
    pub fn show_response(
        &self,
        ui: &Ui,
        id: egui::Id,
        at: egui::Pos2,
        leads_somewhere: bool,
    ) -> (Option<Response>, f32) {
        self.place(ui, id, at, true, leads_somewhere)
    }

    fn place(
        &self,
        ui: &Ui,
        id: egui::Id,
        at: egui::Pos2,
        senses: bool,
        leads_somewhere: bool,
    ) -> (Option<Response>, f32) {
        let galley = elided(ui, self.text, self.font.clone(), self.color, self.width, 1);
        let rect = Rect::from_min_size(at, galley.size());
        let mut response = None;
        if senses && !self.text.is_empty() {
            let text = ui.interact(rect, id, Sense::click());
            if leads_somewhere && text.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter()
                    .hline(rect.x_range(), rect.bottom() - 1.0, (1.0, self.color));
            }
            response = Some(text);
        }
        ui.painter().galley(at, galley, self.color);
        (response, rect.width())
    }
}
