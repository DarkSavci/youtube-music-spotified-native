//! One artist, song or album: your figures for it, in a panel of its own
//! under the lookup.

use eframe::egui::{self, Align2, CornerRadius, Frame, Margin, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{MonthPlays, StatDetail, StatKind, channel_id, cover};

use super::{grouped, listened, plural, tiles};
use crate::actions::Action;
use crate::state::{Loadable, Page, StatSelection, State};
use crate::theme::{self, Icon};
use crate::views::widgets::{self, ArtShape};
use crate::views::{skeleton, tracks};

const ART: f32 = 128.0;
const PLAY_BUTTON: f32 = 44.0;
const PLOT_HEIGHT: f32 = 120.0;
/// The widest a month's bar gets.
const BAR_MOST: f32 = 24.0;
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let Some(selected) = &state.stats_page.selected else {
        return;
    };
    let palette = &state.palette;
    let frame = Frame::new()
        .fill(palette.surface)
        .corner_radius(12)
        .inner_margin(Margin::same(20));
    let panel = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        match &selected.detail {
            Loadable::Loaded(detail) if detail.plays > 0 => {
                contents(state, ui, actions, selected, detail);
            }
            Loadable::NotLoaded | Loadable::Loading => skeleton::panel(state, ui, 180.0),
            Loadable::Loaded(_) | Loadable::Failed(_) => {
                ui.label(
                    egui::RichText::new("No plays of this in your history.")
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
            }
        }
    });
    // The cross sits in the panel's corner, over whatever is under it.
    let corner = panel.response.rect.right_top() + vec2(-26.0, 26.0);
    let close = widgets::IconButton {
        icon: Icon::X,
        size: 18.0,
        tooltip: "Close",
        active: false,
    };
    if close.show_at(ui, palette, corner).clicked() {
        actions.push(Action::CloseStat);
    }
    ui.add_space(24.0);
}

/// "4 Oct 2026", from a time as the core writes it.
fn day(time: &str) -> Option<String> {
    let mut parts = time.get(..10)?.split('-');
    let year = parts.next()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let name = MONTHS.get(month.checked_sub(1)?)?;
    Some(format!("{day} {} {year}", &name[..3]))
}

/// The month a bar stands for, short for the axis ("Sep") and in full for
/// the tooltip ("September 2026").
fn month_names(month: &str) -> (&'static str, String) {
    let (year, number) = month.split_once('-').unwrap_or((month, ""));
    let name = number
        .parse::<usize>()
        .ok()
        .and_then(|number| MONTHS.get(number.checked_sub(1)?))
        .copied()
        .unwrap_or("");
    (&name[..name.len().min(3)], format!("{name} {year}"))
}

fn contents(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    selected: &StatSelection,
    detail: &StatDetail,
) {
    header(state, ui, actions, selected, detail);
    ui.add_space(20.0);
    let (rank_of, songs) = match detail.kind {
        StatKind::Track => ("Among your songs", None),
        StatKind::Artist => ("Among your artists", Some(detail.distinct_tracks)),
        StatKind::Album => ("Among your albums", Some(detail.distinct_tracks)),
    };
    let mut figures = vec![
        ("Plays", grouped(u64::from(detail.plays))),
        ("Listening time", listened(detail.total_ms)),
        ("Last 30 days", plural(detail.plays30d, "play")),
        (rank_of, format!("#{}", detail.rank)),
    ];
    if let Some(songs) = songs {
        figures.push(("Songs played", grouped(u64::from(songs))));
    }
    tiles(state, ui, &figures, true);
    ui.add_space(20.0);
    months(state, ui, &detail.months);
    if selected.tracks.is_empty() {
        return;
    }
    ui.add_space(20.0);
    subtitle(ui, "Your most-played songs");
    let origin = format!("Your top songs: {}", detail.name);
    let list = tracks::List {
        tracks: &selected.tracks,
        origin: &origin,
        editable_playlist: None,
        columns: tracks::Columns {
            cover: true,
            album: false,
        },
        mode: tracks::Mode::List,
    };
    ui.spacing_mut().item_spacing.y = 0.0;
    tracks::table(state, ui, actions, list);
}

fn subtitle(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::semibold(16.0)));
    ui.add_space(12.0);
}

/// The cover, what it is, its name, whose it is, when it was first and
/// last played, and a button that plays it.
fn header(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    selected: &StatSelection,
    detail: &StatDetail,
) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), ART);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let art = Rect::from_min_size(rect.min, vec2(ART, ART));
    let artist = detail.kind == StatKind::Artist;
    // The artist's own picture, when it has been looked up.
    let photo = state
        .artist_photos
        .get(&selected.id)
        .and_then(Option::as_ref)
        .filter(|photo| artist && !photo.is_empty());
    if artist
        && channel_id(&selected.id).is_some()
        && !state.artist_photos.contains_key(&selected.id)
    {
        actions.push(Action::WantArtistPhoto(selected.id.clone()));
    }
    let (shape, placeholder) = if artist {
        (ArtShape::Circle, Icon::User)
    } else {
        (ArtShape::Rounded(8), Icon::Music)
    };
    match photo {
        Some(photo) => widgets::artwork(ui, state, photo, art, shape, placeholder),
        None => widgets::artwork(ui, state, &cover(&detail.artwork), art, shape, placeholder),
    }
    let play = Rect::from_center_size(
        pos2(
            rect.right() - PLAY_BUTTON / 2.0,
            rect.bottom() - PLAY_BUTTON / 2.0,
        ),
        vec2(PLAY_BUTTON, PLAY_BUTTON),
    );
    let left = art.right() + 20.0;
    let width = (play.left() - 16.0 - left).max(60.0);
    // Stacked upwards from the cover's bottom edge.
    let mut bottom = rect.bottom();
    let mut line = |ui: &Ui, text: &str, font: egui::FontId, color| {
        let galley = widgets::elided(ui, text, font, color, width, 1);
        bottom -= galley.size().y + 4.0;
        let at = pos2(left, bottom);
        let size = galley.size();
        ui.painter().galley(at, galley, color);
        Rect::from_min_size(at, size)
    };
    let dates = [
        detail
            .first_played_at
            .as_deref()
            .and_then(day)
            .map(|day| format!("First played {day}")),
        detail
            .last_played_at
            .as_deref()
            .and_then(day)
            .map(|day| format!("Last played {day}")),
    ];
    let dates: Vec<String> = dates.into_iter().flatten().collect();
    if !dates.is_empty() {
        line(
            ui,
            &dates.join(" · "),
            theme::regular(13.0),
            palette.secondary,
        );
    }
    if !artist && !detail.artist.is_empty() {
        let whose = line(ui, &detail.artist, theme::regular(13.0), palette.secondary);
        // Whose it is leads to the listener's figures for them.
        if channel_id(&detail.artist_id).is_some() {
            let response = ui.interact(whose, ui.id().with("stat-artist"), Sense::click());
            widgets::name(ui, &response, &format!("Your listening: {}", detail.artist));
            if response.hovered() {
                let under = whose.bottom() - 1.0;
                ui.painter()
                    .hline(whose.x_range(), under, (1.0, palette.secondary));
            }
            if response.clicked() {
                actions.push(Action::OpenStat {
                    kind: StatKind::Artist,
                    id: detail.artist_id.clone(),
                });
            }
        }
    }
    let name = line(ui, &detail.name, theme::bold(22.0), palette.text);
    // Its name leads to its own page, when it has one.
    let page = match detail.kind {
        StatKind::Artist => channel_id(&detail.id).map(|id| Page::Artist(id.to_owned())),
        StatKind::Album if !detail.album_id.is_empty() => {
            Some(Page::Album(detail.album_id.clone()))
        }
        StatKind::Album | StatKind::Track => None,
    };
    if let Some(page) = page {
        let response = ui.interact(name, ui.id().with("stat-name"), Sense::click());
        widgets::name(ui, &response, &format!("Open {}", detail.name));
        if response.hovered() {
            ui.painter()
                .hline(name.x_range(), name.bottom() - 1.0, (1.0, palette.text));
        }
        if response.clicked() {
            actions.push(Action::Open(page));
        }
    }
    let kind = match detail.kind {
        StatKind::Track => "SONG",
        StatKind::Artist => "ARTIST",
        StatKind::Album => "ALBUM",
    };
    let font = theme::semibold(11.0);
    let kind = widgets::tracked(ui, kind, font, palette.secondary, 0.7, (width, 1));
    let at = pos2(left, bottom - 4.0 - kind.size().y);
    ui.painter().galley(at, kind, palette.secondary);

    let response = ui.interact(play, ui.id().with("stat-play"), Sense::click());
    widgets::name(ui, &response, &format!("Play {}", detail.name));
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.accent, palette.accent_hover, lift);
    let radius = PLAY_BUTTON / 2.0 + lift;
    ui.painter().circle_filled(play.center(), radius, fill);
    widgets::paint_icon(ui, Icon::PlayFilled, play, 18.0, palette.on_accent);
    if response.clicked() {
        actions.push(played(selected, detail));
    }
}

/// What the panel's button plays: the song itself, or the most played
/// songs of the artist or the album.
fn played(selected: &StatSelection, detail: &StatDetail) -> Action {
    if detail.kind != StatKind::Track {
        return Action::Play {
            tracks: selected.tracks.clone(),
            index: 0,
            origin: format!("Your top songs: {}", detail.name),
        };
    }
    let song = spotified_client::models::TrackStat {
        track_id: detail.id.clone(),
        title: detail.name.clone(),
        artist: detail.artist.clone(),
        artist_id: detail.artist_id.clone(),
        plays: detail.plays,
        total_ms: detail.total_ms,
        artwork: detail.artwork.clone(),
    };
    Action::Play {
        tracks: vec![song.track()],
        index: 0,
        origin: "Your listening".to_owned(),
    }
}

/// Plays per month over the last year.
///
/// One series, so no legend: the heading names it. Bars in the accent,
/// thin, rounded at the top and sitting on one baseline; each month has
/// the whole height of the plot to point at, and says its figures when
/// pointed at.
fn months(state: &State, ui: &mut Ui, months: &[MonthPlays]) {
    if months.is_empty() {
        return;
    }
    let palette = &state.palette;
    subtitle(ui, "Plays per month");
    let size = vec2(ui.available_width(), PLOT_HEIGHT);
    let (plot, _) = ui.allocate_exact_size(size, Sense::hover());
    let (axis, _) = ui.allocate_exact_size(vec2(size.x, 20.0), Sense::hover());
    let most = months
        .iter()
        .map(|month| month.plays)
        .max()
        .unwrap_or(1)
        .max(1);
    let column = plot.width() / months.len() as f32;
    ui.painter()
        .hline(plot.x_range(), plot.bottom(), (1.0, palette.outline));
    for (index, month) in months.iter().enumerate() {
        let left = plot.left() + index as f32 * column;
        let whole = Rect::from_min_size(pos2(left, plot.top()), vec2(column, PLOT_HEIGHT));
        let response = ui.interact(whole, ui.id().with(("month", index)), Sense::hover());
        let (short, long) = month_names(&month.month);
        if month.plays > 0 {
            let lift = widgets::hover(ui, &response);
            let height = (month.plays as f32 / most as f32 * PLOT_HEIGHT).max(4.0);
            let width = BAR_MOST.min(column * 0.7);
            let bar = Rect::from_min_max(
                pos2(whole.center().x - width / 2.0, plot.bottom() - height),
                pos2(whole.center().x + width / 2.0, plot.bottom()),
            );
            let round = CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            };
            let fill = palette.accent.gamma_multiply(0.85 + 0.15 * lift);
            ui.painter().rect_filled(bar, round, fill);
        }
        let font = theme::regular(11.0);
        let at = pos2(whole.center().x, axis.center().y);
        widgets::text_at(ui, at, Align2::CENTER_CENTER, short, font, palette.dim);
        response.on_hover_text(format!(
            "{} · {}\n{long}",
            plural(month.plays, "play"),
            listened(month.total_ms)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_from_the_core_is_shown_as_a_day() {
        assert_eq!(day("2026-10-04T18:21:07Z").as_deref(), Some("4 Oct 2026"));
        assert_eq!(day("2026-13-04T00:00:00Z"), None);
        assert_eq!(day("soon"), None);
    }

    #[test]
    fn a_month_is_named_short_for_the_axis_and_in_full_for_the_tooltip() {
        assert_eq!(month_names("2026-09"), ("Sep", "September 2026".to_owned()));
        assert_eq!(month_names("nonsense").0, "");
    }
}
