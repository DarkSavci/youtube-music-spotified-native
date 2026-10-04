//! A person's picture: their own where the account has one, the first
//! letter of their name on a disc until it has loaded or where it has
//! none.

use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, Ui};

use super::{centre_crop, paint_icon, text_at};
use crate::state::State;
use crate::theme::{self, Icon};

/// Who is drawn, and in what colours when there is no picture of them.
pub struct Avatar<'a> {
    pub name: &'a str,
    /// The address of the picture; empty when there is none.
    pub url: &'a str,
    pub fill: Color32,
    pub ink: Color32,
}

impl Avatar<'_> {
    pub fn paint(&self, ui: &Ui, state: &State, rect: Rect) {
        if !ui.is_rect_visible(rect) {
            return;
        }
        let picture = (!self.url.is_empty())
            .then(|| state.images.texture(self.url))
            .flatten();
        if let Some((id, size)) = picture {
            egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
                .uv(centre_crop(size, rect.size()))
                .corner_radius(CornerRadius::same(u8::MAX))
                .paint_at(ui, rect);
            return;
        }
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, self.fill);
        match self.name.chars().next() {
            Some(initial) => {
                let letter: String = initial.to_uppercase().collect();
                let font = theme::bold(rect.height() * 0.42);
                let centre = rect.center();
                text_at(ui, centre, Align2::CENTER_CENTER, &letter, font, self.ink);
            }
            None => paint_icon(ui, Icon::User, rect, rect.height() * 0.5, self.ink),
        }
    }
}
