//! The skin's display: the analyser, the lamps, the time in the skin's
//! digits, the marquee and the sample rate.

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Id, Sense};

use super::super::super::format;
use super::{View, stopped, times};
use crate::actions::Action;
use crate::skin::layout::{self, Area};
use crate::skin::{font, sprites};
use crate::skins::Ask;
use crate::state::{Playback, State};
use crate::theme;

/// How often the analyser moves.
pub(super) const ANALYSER_FRAME: Duration = Duration::from_millis(33);
/// How long the marquee waits between one-character steps.
pub(super) const MARQUEE_STEP: Duration = Duration::from_millis(220);
/// The marquee shows thirty-one characters, the last of them the left half
/// of a thirty-first; text this long fits without scrolling.
const MARQUEE_CHARS: usize = 31;
const MARQUEE_FITS: usize = 30;
/// What separates the end of a scrolling title from its start again.
const MARQUEE_GAP: &str = "  ***  ";
/// The analyser: nineteen bars of three pixels, a pixel apart, sixteen tall.
const BARS: usize = 19;
const ROWS: f32 = 16.0;
/// How fast a bar falls once the sound has left it, and its peak after it,
/// in rows a second.
const BAR_FALL: f32 = 40.0;
const PEAK_FALL: f32 = 9.0;

/// How high each bar of the analyser stands and where its peak hangs, in
/// rows, kept between frames so that both can fall at their own pace.
#[derive(Clone, Default)]
struct Levels {
    bars: [f32; BARS],
    peaks: [f32; BARS],
}

/// The display's left box: the spectrum analyser, the oscilloscope, or
/// nothing, in the skin's own colours. A click, or V, goes to the next.
/// Returns whether anything is still moving.
pub(super) fn analyser(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
) -> bool {
    let area = layout::VISUALIZER;
    if view
        .interact(area, "Analyser display", Sense::click())
        .clicked()
    {
        actions.push(Action::Skin(Ask::CycleAnalyser));
    }
    if !state.settings.skin_analyser {
        return false;
    }
    let palette = view.skin().vis_colors;
    let color = |index: usize| {
        let [red, green, blue] = palette[index.min(palette.len() - 1)];
        Color32::from_rgb(red, green, blue)
    };
    view.fill(area, color(0));
    for y in (0..area.height).step_by(2) {
        for x in (0..area.width).step_by(2) {
            view.fill(Area::new(area.x + x, area.y + y, 1, 1), color(1));
        }
    }
    let sounding = playback.is_some_and(Playback::is_playing);
    if state.settings.skin_scope {
        // Winamp's scope read every seventh sample across its window.
        let wave = match (&state.audio_tap, sounding) {
            (Some(tap), true) => tap.wave(area.width as usize, 7),
            _ => Vec::new(),
        };
        let mut last = scope_row(wave.first().copied().unwrap_or(0.0));
        for x in 0..area.width {
            let row = scope_row(wave.get(x as usize).copied().unwrap_or(0.0));
            // Joined to the last point, so a steep wave is still a line.
            let (top, bottom) = if row > last {
                (last + 1, row)
            } else {
                (row, last.max(row))
            };
            last = row;
            let shade = color(18 + scope_shade(row));
            view.fill(
                Area::new(area.x + x, area.y + top, 1, bottom - top + 1),
                shade,
            );
        }
        return sounding;
    }
    let heard = match (&state.audio_tap, sounding) {
        (Some(tap), true) => tap.spectrum(BARS),
        _ => vec![0.0; BARS],
    };
    let id = Id::new("skin-analyser");
    let passed = view.ui.input(|input| input.stable_dt).min(0.1);
    let mut levels: Levels = view.ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let mut settled = true;
    for (index, now) in heard.iter().enumerate().take(BARS) {
        let bar = (now.clamp(0.0, 1.0) * ROWS).max(levels.bars[index] - BAR_FALL * passed);
        let peak = bar.max(levels.peaks[index] - PEAK_FALL * passed);
        (levels.bars[index], levels.peaks[index]) = (bar, peak);
        settled &= peak < 0.5;
        let x = area.x + 4 * index as u32;
        let height = (bar.round() as u32).min(ROWS as u32);
        for row in (ROWS as u32 - height)..ROWS as u32 {
            view.fill(Area::new(x, area.y + row, 3, 1), color(2 + row as usize));
        }
        let peak = (peak.round() as u32).min(ROWS as u32);
        if peak > 0 {
            let row = ROWS as u32 - peak;
            view.fill(Area::new(x, area.y + row, 3, 1), color(23));
        }
    }
    view.ui.data_mut(|data| data.insert_temp(id, levels));
    sounding || !settled
}

/// The row of the scope a sample is drawn on, of sixteen: the middle for
/// silence, as Winamp placed it.
fn scope_row(sample: f32) -> u32 {
    let byte = (sample * 2.0 * 128.0 + 128.0).round().clamp(0.0, 255.0);
    let row = (byte / 16.0 * 2.0).round() - 9.0;
    row.clamp(0.0, ROWS - 1.0) as u32
}

/// Which of the scope's five colours a row is drawn in: brightest at the
/// middle, darker towards the edges.
fn scope_shade(row: u32) -> usize {
    match row {
        14.. => 4,
        12..=13 => 3,
        10..=11 => 2,
        8..=9 => 1,
        6..=7 => 0,
        4..=5 => 1,
        2..=3 => 2,
        _ => 3,
    }
}

/// The play, pause and stop lamp, the work indicator, and the mono and
/// stereo lamps.
pub(super) fn status(view: &mut View<'_>, playback: Option<&Playback>) {
    let status = match playback {
        Some(playback) if playback.wants_to_play() => sprites::STATUS_PLAYING,
        Some(_) if !stopped(playback) => sprites::STATUS_PAUSED,
        _ => sprites::STATUS_STOPPED,
    };
    view.sprite(status, layout::STATUS);
    // On the way to playing, and not there yet.
    let working =
        playback.is_some_and(|playback| playback.wants_to_play() && !playback.is_playing());
    let work = if working {
        sprites::WORK_INDICATOR_ON
    } else {
        sprites::WORK_INDICATOR_OFF
    };
    view.sprite(work, layout::WORK_INDICATOR);
    let stereo = if playback.is_some() && !stopped(playback) {
        sprites::STEREO_ON
    } else {
        sprites::STEREO_OFF
    };
    view.sprite(stereo, layout::STEREO);
    view.sprite(sprites::MONO_OFF, layout::MONO);
}

/// The time in the skin's digits: elapsed, or remaining with a minus sign,
/// blinking while paused, blank with nothing on.
pub(super) fn time_display(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
) {
    let whole = Area::new(
        layout::MINUS_EX.x,
        layout::MINUS_EX.y,
        layout::SECOND_ONES.x + layout::SECOND_ONES.width - layout::MINUS_EX.x,
        layout::MINUS_EX.height,
    );
    if view.interact(whole, "Time", Sense::click()).clicked() {
        actions.push(Action::ToggleRemainingTime);
    }
    let extended = view.skin().has_extended_digits();
    // The blank digit is painted, not left out: what the main sheet has
    // under the digits is the skin's own idea of an empty display, which
    // is not always empty.
    let blank = |view: &mut View<'_>| {
        if extended {
            for cell in layout::TIME_DIGITS {
                view.sprite(sprites::NUMS_EX_BLANK, cell);
            }
            view.sprite(sprites::NUMS_EX_BLANK, layout::MINUS_EX);
        } else {
            for cell in layout::TIME_DIGITS {
                view.sprite(sprites::NUMBERS_BLANK, cell);
            }
            view.sprite(sprites::NUMBERS_NO_MINUS, layout::MINUS);
        }
    };
    let Some(playback) = playback.filter(|_| !stopped(playback)) else {
        blank(view);
        return;
    };
    let clock = view.ui.input(|input| input.time);
    if !playback.wants_to_play() && (clock * 2.0).floor() as i64 % 2 == 1 {
        blank(view);
        return;
    }
    let (position, duration) = times(view, playback);
    let remaining = state.settings.remaining_time && duration > 0;
    let shown = if remaining {
        duration.saturating_sub(position)
    } else {
        position
    };
    let seconds = (shown / 1000) as u32;
    let (minutes, seconds) = ((seconds / 60).min(99), seconds % 60);
    let digits = [minutes / 10, minutes % 10, seconds / 10, seconds % 10];
    for (value, cell) in digits.into_iter().zip(layout::TIME_DIGITS) {
        let sprite = if extended {
            sprites::digit_ex(value)
        } else {
            sprites::digit(value)
        };
        view.sprite(sprite, cell);
    }
    match (extended, remaining) {
        (true, true) => view.sprite(sprites::NUMS_EX_MINUS, layout::MINUS_EX),
        (true, false) => view.sprite(sprites::NUMS_EX_BLANK, layout::MINUS_EX),
        (false, true) => view.sprite(sprites::NUMBERS_MINUS, layout::MINUS),
        (false, false) => view.sprite(sprites::NUMBERS_NO_MINUS, layout::MINUS),
    }
}

/// What the marquee says: a slider while it moves, as Winamp announced
/// them, then anything the app has to say, else the song.
fn marquee_text(
    state: &State,
    playback: Option<&Playback>,
    seeking: Option<f32>,
    volume: Option<f32>,
    balance: Option<f32>,
) -> String {
    if let Some(balance) = balance {
        let percent = (balance.abs() * 100.0).round() as u32;
        return match balance {
            side if side < 0.0 => format!("Balance: {percent}% left"),
            side if side > 0.0 => format!("Balance: {percent}% right"),
            _ => "Balance: center".to_owned(),
        };
    }
    if let Some(volume) = volume {
        return format!("Volume: {}%", (volume * 100.0).round() as u32);
    }
    let track = playback.and_then(Playback::current);
    if let (Some(fraction), Some(track)) = (seeking, track)
        && track.duration_ms > 0
    {
        let target = (fraction * track.duration_ms as f32) as u64;
        return format!(
            "Seek to: {}/{} ({}%)",
            format::duration(target),
            format::duration(track.duration_ms),
            (fraction * 100.0).round() as u32
        );
    }
    // The app's notices (a skin added, a song that would not play) have no
    // toast to live in here; Winamp used the marquee for its own.
    if let Some(toast) = state.toasts.last() {
        return toast.text.clone();
    }
    let Some(track) = track else {
        return "Youtube Music Spotified".to_owned();
    };
    let artists = track.artist_names();
    let mut text = if artists.is_empty() {
        track.title.clone()
    } else {
        format!("{artists} - {}", track.title)
    };
    if track.duration_ms > 0 {
        text.push_str(&format!(" ({})", format::duration(track.duration_ms)));
    }
    text
}

/// Where the marquee has got to, kept between frames.
#[derive(Clone)]
struct Scroll {
    text: String,
    /// How many characters it has moved along.
    offset: usize,
    moved: Instant,
}

/// How many characters along the marquee is, for this text, now. A new
/// text starts from its beginning.
fn scrolled(view: &View<'_>, text: &str) -> usize {
    let id = Id::new("skin-marquee");
    let now = Instant::now();
    let kept: Option<Scroll> = view.ui.data(|data| data.get_temp(id));
    let mut scroll = kept
        .filter(|scroll| scroll.text == text)
        .unwrap_or_else(|| Scroll {
            text: text.to_owned(),
            offset: 0,
            moved: now,
        });
    let passed = now.saturating_duration_since(scroll.moved);
    let steps = (passed.as_millis() / MARQUEE_STEP.as_millis()) as u32;
    if steps > 0 {
        scroll.offset = scroll.offset.wrapping_add(steps as usize);
        scroll.moved += MARQUEE_STEP * steps;
    }
    let offset = scroll.offset;
    view.ui.data_mut(|data| data.insert_temp(id, scroll));
    offset
}

pub(super) fn marquee(state: &State, view: &mut View<'_>, playback: Option<&Playback>) {
    let most = state.settings.max_volume();
    let volume = view.held("Volume").map(|fraction| fraction * most);
    let balance = view.held("Balance").map(super::balance_of);
    let text = marquee_text(state, playback, view.held("Seek"), volume, balance);
    let offset = scrolled(view, &text);
    if !text.chars().all(font::covered) {
        // The skin's bitmap font cannot say this (a Japanese title would
        // come out as question marks): the line is set in the app's own
        // face instead, in the playlist's colour, and slid past the same.
        return marquee_in_type(view, &text, offset);
    }
    let mut characters: Vec<char> = text.chars().collect();
    if characters.len() <= MARQUEE_FITS {
        return view.text(&text, layout::MARQUEE);
    }
    characters.extend(MARQUEE_GAP.chars());
    let shown: String = (0..MARQUEE_CHARS)
        .map(|index| characters[(offset + index) % characters.len()])
        .collect();
    view.text(&shown, layout::MARQUEE);
}

/// The marquee set in the app's type, for what the skin's font lacks.
fn marquee_in_type(view: &View<'_>, text: &str, offset: usize) {
    let area = layout::MARQUEE;
    let rect = view.rect(area);
    let [red, green, blue] = view.skin().playlist.normal;
    let color = Color32::from_rgb(red, green, blue);
    // A little taller than the bitmap font's six pixels: type needs room
    // for what hangs under the line.
    let face = theme::medium((area.height as f32 + 2.0) * view.unit);
    let painter = view
        .ui
        .painter()
        .with_clip_rect(rect.expand2(egui::vec2(0.0, 2.0 * view.unit)));
    let lay = |text: String| painter.layout_no_wrap(text, face.clone(), color);
    let still = lay(text.to_owned());
    let top = rect.center().y - still.size().y / 2.0;
    if still.size().x <= rect.width() {
        painter.galley(egui::pos2(rect.left(), top), still, color);
        return;
    }
    let strip = lay(format!("{text}{MARQUEE_GAP}"));
    let width = strip.size().x;
    let along = (offset as f32 * 5.0 * view.unit) % width;
    for copy in 0..2 {
        let left = rect.left() - along + copy as f32 * width;
        if left < rect.right() {
            painter.galley(egui::pos2(left, top), strip.clone(), color);
        }
    }
}

/// The bitrate, which is the stream's size over its length, and the sample
/// rate, which is the device's. A stream whose size is not known shows no
/// bitrate rather than a guess.
pub(super) fn rates(state: &State, view: &mut View<'_>, playback: Option<&Playback>) {
    if playback.is_none() || stopped(playback) {
        return;
    }
    if let Some(tap) = &state.audio_tap {
        let kilobits = tap.bitrate();
        if kilobits > 0 {
            view.text(&format!("{:>3}", kilobits.min(999)), layout::KBPS);
        }
        let kilohertz = tap.sample_rate() / 1000;
        view.text(&format!("{kilohertz:>2}"), layout::KHZ);
    }
}
