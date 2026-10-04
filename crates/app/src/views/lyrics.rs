//! Lyrics: the words of what is playing, following the song when they are
//! timed. A panel on the right, or the whole window.
//!
//! Both sit on the cover's colour. Every line is bold; the one being sung
//! is white and the rest are white seen through the colour, so the eye
//! finds its place without the text changing size or jumping about.

use std::time::Instant;

use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::Lyrics;

use super::widgets::{self, ArtShape};
use super::{chrome, player_bar};
use crate::actions::Action;
use crate::state::{Loadable, Playback, State};
use crate::theme::{self, Icon};

const WIDTH: f32 = 360.0;
const WIDEST: f32 = 640.0;
/// The cover the player bar draws, whose colour the panel takes.
const BAR_COVER: f32 = 56.0;
/// The cover in the full view's header.
const HEADER_COVER: f32 = 48.0;
const HEADER_HEIGHT: f32 = 80.0;
/// The transport along the bottom of the full view.
const FOOTER_HEIGHT: f32 = 96.0;
/// How strongly the lines that are not being sung show.
const QUIET: f32 = 0.45;

/// How lines are drawn, which is all that differs between the panel and the
/// full view.
struct LineStyle {
    size: f32,
    gap: f32,
    /// Empty room above the first line and below the last, as a share of
    /// the height on show, so they too can be brought to the middle.
    margin: f32,
}

/// The colour lyrics sit on: the cover's, or the panel's until it loads.
fn background(state: &State, ui: &Ui, cover: f32) -> Color32 {
    state
        .playback
        .as_ref()
        .and_then(Playback::current)
        .and_then(|track| widgets::artwork_tint(ui, state, &track.artwork, cover))
        .unwrap_or(state.palette.panel)
}

/// `most` is the widest the panel may be in this window.
pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, most: f32) {
    let palette = &state.palette;
    let frame = widgets::card(palette, super::RIGHT_PANEL_GUTTERS)
        .fill(background(state, ui, BAR_COVER))
        .inner_margin(Margin::same(12));
    egui::Panel::right("lyrics")
        .resizable(true)
        .show_separator_line(false)
        .default_size(WIDTH.min(most))
        .size_range(super::RIGHT_PANEL_MIN..=most.min(WIDEST))
        .frame(frame)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Lyrics").font(theme::bold(16.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if widgets::icon_button(ui, palette, Icon::X, 16.0, "Close").clicked() {
                        actions.push(Action::ToggleLyrics);
                    }
                    if widgets::icon_button(ui, palette, Icon::Expand, 16.0, "Full screen")
                        .clicked()
                    {
                        actions.push(Action::SetLyricsFullscreen(true));
                    }
                });
            });
            ui.add_space(8.0);
            let style = LineStyle {
                size: 20.0,
                gap: 12.0,
                margin: 0.08,
            };
            Frame::new()
                .inner_margin(Margin::symmetric(8, 0))
                .show(ui, |ui| words(state, ui, actions, &style));
        });
}

/// The words alone, smaller, for the mini player.
pub(super) fn compact(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let style = LineStyle {
        size: 16.0,
        gap: 12.0,
        margin: 0.25,
    };
    Frame::new()
        .inner_margin(Margin::symmetric(8, 0))
        .show(ui, |ui| words(state, ui, actions, &style));
}

/// The whole window given to the lyrics: the cover's colour edge to edge,
/// what is playing named at the top, and the transport along the bottom in
/// place of the player bar.
pub fn fullscreen(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let fill = background(state, ui, HEADER_COVER);
    egui::CentralPanel::default()
        .frame(Frame::new().fill(fill))
        .show(ui, |ui| {
            let area = ui.max_rect();
            let (header, rest) = area.split_top_bottom_at_y(area.top() + HEADER_HEIGHT);
            let (body, footer) = rest.split_top_bottom_at_y(rest.bottom() - FOOTER_HEIGHT);
            // The margins grow with the window, as the old view's did.
            let side = (area.width() * 0.08).clamp(24.0, 120.0);

            chrome::drag(state, ui, header);
            self::header(state, ui, actions, header.shrink2(vec2(24.0, 16.0)));

            let style = LineStyle {
                size: (area.width() * 0.032).clamp(24.0, 34.0),
                gap: 16.0,
                margin: 0.4,
            };
            let column = body.shrink2(vec2(side, 0.0));
            let mut column_ui = ui.new_child(egui::UiBuilder::new().max_rect(column));
            words(state, &mut column_ui, actions, &style);

            // Darker towards the bottom, so the transport reads on any cover.
            let shade = Color32::from_black_alpha(90);
            widgets::vertical_gradient(ui, footer, Color32::TRANSPARENT, shade);
            let playback = state
                .playback
                .as_ref()
                .filter(|playback| playback.current().is_some());
            ui.add_enabled_ui(playback.is_some(), |ui| {
                let zone = footer.shrink2(vec2(side, 0.0));
                player_bar::transport(state, ui, actions, playback, zone);
            });
        });
}

/// What is playing, small, and the way back out.
fn header(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect) {
    let palette = &state.palette;
    let leave = widgets::IconButton {
        icon: Icon::Shrink,
        size: 18.0,
        tooltip: "Leave full screen",
        active: false,
    };
    let corner = pos2(area.right() - 15.0, area.center().y);
    let left = leave.show_at(ui, palette, corner).clicked();
    if left || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        actions.push(Action::SetLyricsFullscreen(false));
    }
    let Some(track) = state.playback.as_ref().and_then(Playback::current) else {
        return;
    };
    let cover = Rect::from_min_size(
        pos2(area.left(), area.center().y - HEADER_COVER / 2.0),
        egui::Vec2::splat(HEADER_COVER),
    );
    let shape = ArtShape::Rounded(4);
    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
    let text_left = cover.right() + 14.0;
    let width = (area.right() - text_left - 48.0).max(20.0);
    let title = widgets::elided(
        ui,
        &track.title,
        theme::semibold(14.0),
        Color32::WHITE,
        width,
        1,
    );
    let at = pos2(text_left, area.center().y - 18.0);
    ui.painter().galley(at, title, Color32::WHITE);
    let quiet = Color32::WHITE.gamma_multiply(0.75);
    let font = theme::regular(12.5);
    let artists = widgets::elided(ui, &track.artist_names(), font, quiet, width, 1);
    let at = pos2(text_left, area.center().y + 2.0);
    ui.painter().galley(at, artists, quiet);
}

/// The lyrics of what is playing, or what there is to say instead.
fn words(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, style: &LineStyle) {
    let palette = &state.palette;
    let playback = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some());
    let Some(playback) = playback else {
        return note(
            state,
            ui,
            "Nothing playing",
            "Play something to see its lyrics.",
        );
    };
    match &state.lyrics.words {
        Loadable::NotLoaded | Loadable::Loading => widgets::loading(ui, palette, "Loading…"),
        Loadable::Failed(message) => widgets::error(ui, palette, message),
        Loadable::Loaded(None) => note(
            state,
            ui,
            "No lyrics",
            "Neither YouTube Music nor LRCLIB has words for this track.",
        ),
        Loadable::Loaded(Some(lyrics)) => lines(ui, actions, playback, lyrics, style),
    }
}

fn note(state: &State, ui: &mut Ui, title: &str, body: &str) {
    widgets::empty_state(ui, &state.palette, Icon::MicVocal, title, body);
}

fn lines(
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    lyrics: &Lyrics,
    style: &LineStyle,
) {
    let position = playback.position_ms(Instant::now());
    let sung = lyrics.line_at(position);
    // The line being sung is brought to the middle when it changes, not
    // every frame, so the words can still be scrolled by hand between lines.
    let memory = ui.id().with("line-in-view");
    let in_view = ui.data(|data| data.get_temp::<usize>(memory));
    egui::ScrollArea::vertical()
        .id_salt("lyrics-lines")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let margin = ui.clip_rect().height() * style.margin;
            ui.add_space(margin);
            ui.spacing_mut().item_spacing.y = style.gap;
            for (index, line) in lyrics.lines.iter().enumerate() {
                let current = sung == Some(index);
                // A timed line with no words is a musical break.
                let text = if line.text.trim().is_empty() {
                    "♪"
                } else {
                    &line.text
                };
                let sense = if lyrics.synced {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                // Laid out once to learn where the line sits, then painted
                // in a colour that depends on whether the pointer is on it.
                let font = theme::bold(style.size);
                let width = ui.available_width();
                let galley =
                    ui.painter()
                        .layout(text.to_owned(), font, Color32::PLACEHOLDER, width);
                let (rect, response) = ui.allocate_exact_size(galley.size(), sense);
                widgets::name(ui, &response, text);
                let strength = match (current, lyrics.synced) {
                    (true, _) => 1.0,
                    // A timed line lights up under the pointer: it can be
                    // clicked to jump there.
                    (false, true) => QUIET + (1.0 - QUIET) * widgets::hover(ui, &response),
                    // Untimed lyrics are read, not followed: all one colour.
                    (false, false) => 0.92,
                };
                if ui.is_rect_visible(rect) {
                    let color = Color32::WHITE.gamma_multiply(strength);
                    ui.painter().galley(rect.min, galley, color);
                }
                if response.clicked() {
                    actions.push(Action::Seek(line.at_ms));
                }
                if current && in_view != Some(index) {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    ui.data_mut(|data| data.insert_temp(memory, index));
                }
            }
            if !lyrics.source.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("Source: {}", lyrics.source))
                        .font(theme::regular(11.5))
                        .color(Color32::WHITE.gamma_multiply(0.6)),
                );
            }
            ui.add_space(margin);
        });
}
