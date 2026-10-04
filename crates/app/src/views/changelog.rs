//! What's new: every release's notes, newest first.

use eframe::egui::{self, Color32, Ui};

use super::{pages, widgets};
use crate::changelog::{self, Release};
use crate::state::State;
use crate::theme::{self, Icon};

/// Notes are read as a column, not across the window.
const MAX_WIDTH: f32 = 720.0;

pub fn show(state: &State, ui: &mut Ui) {
    pages::title(ui, "What's new", 30.0);
    // No wider than reads well, and never wider than there is room for.
    ui.set_max_width(MAX_WIDTH.min(ui.available_width()));
    let releases = changelog::releases();
    if releases.is_empty() {
        let text = "This build came without its notes.";
        widgets::empty_state(ui, &state.palette, Icon::Clock, "No release notes", text);
        return;
    }
    for entry in releases {
        release(state, ui, entry);
    }
}

fn release(state: &State, ui: &mut Ui, release: &Release) {
    let palette = &state.palette;
    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&release.version).font(theme::bold(22.0)));
        if release.version == changelog::VERSION {
            ui.label(
                egui::RichText::new("Installed")
                    .font(theme::semibold(11.5))
                    .color(palette.accent),
            );
        }
        ui.label(
            egui::RichText::new(&release.date)
                .font(theme::regular(12.5))
                .color(palette.secondary),
        );
    });
    if !release.note.is_empty() {
        ui.label(
            egui::RichText::new(&release.note)
                .font(theme::regular(14.0))
                .color(palette.secondary),
        );
    }
    for group in &release.groups {
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(&group.title)
                .font(theme::semibold(13.0))
                .color(kind_color(state, &group.title)),
        );
        ui.add_space(2.0);
        for item in &group.items {
            ui.horizontal_top(|ui| {
                ui.label(egui::RichText::new("•").color(palette.dim));
                ui.add(
                    egui::Label::new(egui::RichText::new(item).font(theme::regular(14.0))).wrap(),
                );
            });
        }
    }
}

/// Each kind of change has its colour, so a release reads at a glance.
fn kind_color(state: &State, title: &str) -> Color32 {
    match title {
        "New" => state.palette.accent,
        "Fixed" => state.palette.warning,
        _ => state.palette.secondary,
    }
}
