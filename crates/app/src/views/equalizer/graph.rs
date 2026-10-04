//! The equalizer's graph: the curve the filters really give, drawn behind
//! the ten sliders that set it, and the preamp beside them.
//!
//! The sliders and the curve share one set of axes. Frequency runs along
//! the bottom on the scale hearing has, so the bands, an octave apart, are
//! evenly spaced; level runs up the side. A slider's knob is therefore a
//! point on the graph, and the curve passes through it.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Event, Key, Modifiers, MouseWheelUnit, Pos2, Rect,
    Response, Sense, Shape, Stroke, Ui, pos2, vec2,
};
use spotified_audio::eq::{self, BANDS, Design, RANGE_DB};

use super::Place;
use crate::actions::Action;
use crate::equalizer::{Ask, STEP_DB, band_label, decibels, stepped};
use crate::state::State;
use crate::theme::{self, Palette};
use crate::views::widgets;

/// The room at the left for the level's scale.
const SCALE_WIDTH: f32 = 30.0;
/// The preamp's own column, and the gap that sets it apart.
const PREAMP_WIDTH: f32 = 48.0;
const PREAMP_GAP: f32 = 10.0;
/// Under the graph: each band's frequency, and under that its level.
const LABELS_HEIGHT: f32 = 38.0;
/// A slider's travel stops this far inside the graph, so that a knob at
/// either end is whole.
const TRAVEL_INSET: f32 = 14.0;
const KNOB: f32 = 6.0;
/// The curve is worked out at a point every this many points across.
const CURVE_STEP: f32 = 3.0;
/// How long the knobs take to reach a preset that was chosen.
const GLIDE: f32 = 0.18;
/// What PageUp and PageDown move a slider by, in decibels.
const BIG_STEP_DB: f32 = 3.0;
/// The lines across the graph, in decibels.
const GRID: [f32; 5] = [12.0, 6.0, 0.0, -6.0, -12.0];
/// The sample rate the curve is drawn for until a device has been opened.
const USUAL_RATE: u32 = 48_000;

/// Which slider: one of the bands, or the preamp.
#[derive(Clone, Copy, PartialEq)]
enum Which {
    Band(usize),
    Preamp,
}

impl Which {
    /// What a screen reader, and a test, calls it.
    fn name(self) -> String {
        match self {
            Which::Band(band) if BANDS[band] >= 1000.0 => {
                format!("{} kHz", BANDS[band] / 1000.0)
            }
            Which::Band(band) => format!("{} Hz", BANDS[band]),
            Which::Preamp => "Preamp".to_owned(),
        }
    }

    fn ask(self, decibels: f32) -> Ask {
        match self {
            Which::Band(band) => Ask::Band(band, decibels),
            Which::Preamp => Ask::Preamp(decibels),
        }
    }
}

/// A slider this frame: where its knob is drawn, and how it is being used.
struct Knob {
    /// The level shown, in decibels: on its way to the setting while a
    /// preset glides in, under the pointer while it is dragged.
    shown: f32,
    centre_x: f32,
    /// How far the pointer's arrival has got, from 0 to 1.
    lit: f32,
    held: bool,
    focused: bool,
}

/// Where levels fall on the graph.
#[derive(Clone, Copy)]
struct Travel {
    zero_y: f32,
    /// The points from nought to the top of the range.
    half: f32,
}

impl Travel {
    fn y(self, decibels: f32) -> f32 {
        self.zero_y - decibels / RANGE_DB * self.half
    }

    fn decibels(self, y: f32) -> f32 {
        (self.zero_y - y) / self.half * RANGE_DB
    }
}

/// The curve as it was last worked out, kept until a slider moves: the
/// panel is drawn many times between two changes, and a hundred points
/// through ten filters each is not worth doing again for the same answer.
#[derive(Clone)]
struct Drawn {
    key: Depends,
    design: Arc<Design>,
    /// Decibels at each point across the graph.
    levels: Arc<[f32]>,
    /// What the level is turned down or up by, with the preamp.
    gain_db: f32,
}

/// What a drawn curve depends on, to tell when it must be drawn again.
#[derive(Clone, Copy, PartialEq)]
struct Depends {
    gains: [u32; 10],
    preamp: u32,
    headroom: bool,
    rate: u32,
    points: usize,
}

/// The level the equalizer adds or takes off overall, for the note beside
/// the headroom switch. From the curve last drawn in this panel.
pub(super) fn gain_db(ui: &Ui) -> f32 {
    ui.data(|data| data.get_temp::<Drawn>(cache_id(ui)))
        .map_or(0.0, |drawn| drawn.gain_db)
}

fn cache_id(ui: &Ui) -> egui::Id {
    egui::Id::new("equalizer-curve").with(ui.layer_id())
}

/// The graph, the sliders and their labels, across the width there is.
pub(super) fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, place: Place) {
    let palette = &state.palette;
    let settings = &state.settings;
    let on = settings.equalizer_on;
    let room = ui.available_width() - SCALE_WIDTH - PREAMP_GAP - PREAMP_WIDTH;
    let column = room / BANDS.len() as f32;
    let width = column * BANDS.len() as f32;
    let height = place.graph_height();
    let size = vec2(ui.available_width(), height + LABELS_HEIGHT);
    let (area, _) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(area) {
        return;
    }
    let plot = Rect::from_min_size(area.min + vec2(SCALE_WIDTH, 0.0), vec2(width, height));
    let preamp = Rect::from_min_size(
        pos2(plot.right() + PREAMP_GAP, plot.top()),
        vec2(PREAMP_WIDTH, height),
    );
    let travel = Travel {
        zero_y: plot.center().y,
        half: height / 2.0 - TRAVEL_INSET,
    };

    // The sliders first: what they are doing decides what is drawn.
    let knobs: [Knob; 10] = std::array::from_fn(|band| {
        let left = plot.left() + column * band as f32;
        let rect = Rect::from_min_size(pos2(left, plot.top()), vec2(column, height));
        let value = settings.equalizer[band];
        slider(ui, actions, place, (rect, travel), value, Which::Band(band))
    });
    let level = settings.equalizer_preamp;
    let pre = slider(ui, actions, place, (preamp, travel), level, Which::Preamp);

    let rate = state
        .audio_tap
        .as_ref()
        .map_or(USUAL_RATE, |tap| tap.sample_rate());
    let shown = eq::Settings {
        enabled: true,
        gains: std::array::from_fn(|band| knobs[band].shown),
        preamp: pre.shown,
        headroom: settings.equalizer_headroom,
    };
    let points = (width / CURVE_STEP).ceil() as usize + 1;
    let drawn = curve(ui, &shown, rate, points);

    let wash = widgets::wash(ui, 0.45);
    for rect in [plot, preamp] {
        ui.painter().rect_filled(rect, CornerRadius::same(8), wash);
    }
    grid(ui, palette, plot, preamp, travel);
    let track = Stroke::new(2.0, widgets::wash(ui, 0.55));
    for knob in knobs.iter().chain([&pre]) {
        let (top, bottom) = (travel.y(RANGE_DB), travel.y(-RANGE_DB));
        let line = [pos2(knob.centre_x, top), pos2(knob.centre_x, bottom)];
        ui.painter().line_segment(line, track);
    }
    trace(ui, palette, plot, travel, &drawn.levels, on);
    // The preamp has no curve; a bar from nought says which way it goes.
    let bar = [
        pos2(pre.centre_x, travel.zero_y),
        pos2(pre.centre_x, travel.y(pre.shown)),
    ];
    let colour = if on { palette.accent } else { palette.dim };
    ui.painter().line_segment(bar, Stroke::new(2.0, colour));

    for knob in knobs.iter().chain([&pre]) {
        paint_knob(ui, palette, knob, travel);
    }
    for (band, knob) in knobs.iter().enumerate() {
        legend(ui, palette, knob, plot.bottom(), &band_label(BANDS[band]));
    }
    legend(ui, palette, &pre, plot.bottom(), "Preamp");
    // Last, over everything: the level under the hand.
    for knob in knobs.iter().chain([&pre]) {
        if knob.held || knob.lit > 0.5 {
            bubble(ui, palette, knob, travel, area);
        }
    }
}

/// The curve for these settings: the one kept, or worked out afresh when
/// anything it depends on has changed.
fn curve(ui: &Ui, shown: &eq::Settings, rate: u32, points: usize) -> Drawn {
    let key = Depends {
        gains: shown.gains.map(f32::to_bits),
        preamp: shown.preamp.to_bits(),
        headroom: shown.headroom,
        rate,
        points,
    };
    let id = cache_id(ui);
    let kept = ui.data(|data| data.get_temp::<Drawn>(id));
    if let Some(kept) = kept.as_ref().filter(|kept| kept.key == key) {
        return kept.clone();
    }
    // The design depends on the device alone, and outlives the curve.
    let design = kept
        .filter(|kept| kept.key.rate == rate)
        .map_or_else(|| Arc::new(Design::new(rate)), |kept| kept.design);
    let shape = design.shape(shown);
    let (lowest, highest) = span();
    let octaves = (highest / lowest).log2();
    // Past half the sample rate there is nothing to draw.
    let limit = rate as f32 * 0.49;
    let levels = (0..points)
        .map(|point| {
            let along = point as f32 / (points - 1).max(1) as f32;
            shape.db_at((lowest * 2f32.powf(octaves * along)).min(limit))
        })
        .collect();
    let drawn = Drawn {
        key,
        design,
        levels,
        gain_db: shape.gain_db,
    };
    ui.data_mut(|data| data.insert_temp(id, drawn.clone()));
    drawn
}

/// The frequencies at the graph's two edges: half an octave past the
/// first and last bands, so that every band has a column of its own.
fn span() -> (f32, f32) {
    let half_octave = std::f32::consts::SQRT_2;
    (BANDS[0] / half_octave, BANDS[BANDS.len() - 1] * half_octave)
}

/// The lines across, and the scale at the left.
fn grid(ui: &Ui, palette: &Palette, plot: Rect, preamp: Rect, travel: Travel) {
    for level in GRID {
        let y = travel.y(level);
        // Nought is the line everything is measured from.
        let colour = if level == 0.0 {
            palette.dim.gamma_multiply(0.7)
        } else {
            palette.outline
        };
        for rect in [plot, preamp] {
            let line = [pos2(rect.left() + 6.0, y), pos2(rect.right() - 6.0, y)];
            ui.painter().line_segment(line, Stroke::new(1.0, colour));
        }
        let label = if level > 0.0 {
            format!("+{level}")
        } else if level < 0.0 {
            format!("−{}", -level)
        } else {
            "0".to_owned()
        };
        let at = pos2(plot.left() - 8.0, y);
        let font = theme::regular(10.0);
        widgets::text_at(ui, at, Align2::RIGHT_CENTER, &label, font, palette.dim);
    }
}

/// The curve, and the wash between it and nought. Switched off it is an
/// outline only: the shape that would be heard, not one that is.
fn trace(ui: &Ui, palette: &Palette, plot: Rect, travel: Travel, levels: &[f32], on: bool) {
    if levels.len() < 2 {
        return;
    }
    let step = plot.width() / (levels.len() - 1) as f32;
    let (top, bottom) = (plot.top() + 2.0, plot.bottom() - 2.0);
    let line: Vec<Pos2> = levels
        .iter()
        .enumerate()
        .map(|(point, level)| {
            let y = travel.y(*level).clamp(top, bottom);
            pos2(plot.left() + step * point as f32, y)
        })
        .collect();
    if on {
        let fill = palette.accent.gamma_multiply(0.16);
        let mut mesh = egui::Mesh::default();
        for (point, at) in line.iter().enumerate() {
            mesh.colored_vertex(*at, fill);
            mesh.colored_vertex(pos2(at.x, travel.zero_y), fill);
            if point > 0 {
                let base = (point as u32 - 1) * 2;
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base + 1, base + 2, base + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));
    }
    let colour = if on { palette.accent } else { palette.dim };
    ui.painter()
        .add(Shape::line(line, Stroke::new(2.0, colour)));
}

fn paint_knob(ui: &Ui, palette: &Palette, knob: &Knob, travel: Travel) {
    let at = pos2(knob.centre_x, travel.y(knob.shown));
    let lit = if knob.held { 1.0 } else { knob.lit };
    // A ring of the panel's own colour parts the knob from the curve.
    let radius = KNOB + lit * 1.5;
    let ground = if ui.visuals().dark_mode {
        Color32::from_black_alpha(90)
    } else {
        Color32::from_white_alpha(200)
    };
    ui.painter().circle_filled(at, radius + 2.0, ground);
    ui.painter().circle_filled(at, radius, palette.text);
    if knob.focused || lit > 0.0 {
        let amount = if knob.focused { 1.0 } else { lit };
        let ring = Stroke::new(2.0, palette.accent.gamma_multiply(amount));
        ui.painter().circle_stroke(at, radius + 1.0, ring);
    }
}

/// A slider's name under it, and under that its level.
fn legend(ui: &Ui, palette: &Palette, knob: &Knob, top: f32, label: &str) {
    let active = knob.held || knob.lit > 0.5 || knob.focused;
    let colour = if active {
        palette.text
    } else {
        palette.secondary
    };
    let at = pos2(knob.centre_x, top + 8.0);
    let font = theme::medium(11.0);
    widgets::text_at(ui, at, Align2::CENTER_TOP, label, font, colour);
    // The level without its unit: ten of them in a row have no room for
    // it, and the scale at the left says what they are.
    let level = decibels(knob.shown);
    let level = level.trim_end_matches(" dB");
    let colour = if knob.shown.abs() < STEP_DB / 2.0 {
        palette.dim
    } else {
        palette.secondary
    };
    let at = pos2(knob.centre_x, top + 23.0);
    let font = theme::regular(10.5);
    widgets::text_at(ui, at, Align2::CENTER_TOP, level, font, colour);
}

/// The level in full, beside the knob that is under the hand.
fn bubble(ui: &Ui, palette: &Palette, knob: &Knob, travel: Travel, within: Rect) {
    let text = decibels(knob.shown);
    let font = theme::semibold(11.0);
    let words = ui.painter().layout_no_wrap(text, font, palette.window);
    let size = words.size() + vec2(14.0, 8.0);
    let y = travel.y(knob.shown);
    // Above the knob, or under it where there is no room above.
    let above = y - KNOB - 10.0 - size.y / 2.0;
    let centre_y = if above - size.y / 2.0 < within.top() {
        y + KNOB + 10.0 + size.y / 2.0
    } else {
        above
    };
    let half = size.x / 2.0;
    let centre_x = knob
        .centre_x
        .clamp(within.left() + half, within.right() - half);
    let rect = Rect::from_center_size(pos2(centre_x, centre_y), size);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(u8::MAX), palette.text);
    let at = rect.center() - words.size() / 2.0;
    ui.painter().galley(at, words, palette.window);
}

/// One slider: the whole of its column answers, so the knob need not be
/// found before it is moved. A press sets it where the pointer is and a
/// drag carries it; two clicks, or a right click, put it back to nought;
/// the wheel and the arrow keys move it a step.
fn slider(
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    place: Place,
    (rect, travel): (Rect, Travel),
    value: f32,
    which: Which,
) -> Knob {
    let name = which.name();
    let id = ui.id().with(("equalizer", &name));
    let response = ui.interact(rect, id, Sense::click_and_drag());
    response
        .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(value), name.as_str()));
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let primary = ui.input(|input| input.pointer.primary_down());
    let held = primary && (response.dragged() || response.is_pointer_button_down_on());
    let focused = response.has_focus();

    let mut wanted = None;
    // Whether what was asked for is to be written down now: a drag is
    // written down once, when it ends.
    let mut keep = false;
    if response.double_clicked() || response.secondary_clicked() {
        wanted = Some(0.0);
        keep = true;
    } else if held || (response.clicked() && response.hovered()) {
        // A click quick enough to be down and up within one frame is
        // never seen held, and sets the slider all the same.
        response.request_focus();
        if let Some(pointer) = response.interact_pointer_pos() {
            wanted = Some(stepped(travel.decibels(pointer.y)));
        }
    }
    if response.drag_stopped() || response.clicked() {
        keep = true;
    }
    if !held {
        let nudge = keys(ui, &response) + wheel(ui, &response, place);
        if nudge != 0.0 {
            wanted = Some(stepped(value + nudge));
            keep = true;
        }
    }
    // Asked for once: a pointer held still, or let go where it was, asks
    // for nothing more, even before the setting has caught up with it.
    let last = id.with("asked");
    let asked = ui.data(|data| data.get_temp::<f32>(last));
    if !held && asked.is_some() {
        ui.data_mut(|data| data.remove::<f32>(last));
    }
    let wanted = wanted.filter(|wanted| *wanted != value);
    if let Some(wanted) = wanted.filter(|wanted| asked != Some(*wanted)) {
        actions.push(Action::Equalizer(which.ask(wanted)));
        if held {
            ui.data_mut(|data| data.insert_temp(last, wanted));
        }
    }
    if keep {
        actions.push(Action::Equalizer(Ask::Keep));
    }

    // Under the hand the knob is where the hand is; otherwise it glides
    // to the setting, which is how a preset is seen to arrive.
    let target = wanted.unwrap_or(value);
    let time = if held || widgets::still(ui.ctx()) {
        0.0
    } else {
        GLIDE
    };
    let shown = ui
        .ctx()
        .animate_value_with_time(id.with("shown"), target, time);
    Knob {
        shown,
        centre_x: rect.center().x,
        lit: widgets::hover_of(ui, id, response.hovered() || held),
        held,
        focused,
    }
}

/// What the arrow keys ask of the slider that has the keyboard, in
/// decibels. The keys are taken, so nothing else answers them as well.
fn keys(ui: &Ui, response: &Response) -> f32 {
    if !response.has_focus() {
        return 0.0;
    }
    // Up and down are the slider's own, not a move to the next control.
    let filter = egui::EventFilter {
        vertical_arrows: true,
        ..Default::default()
    };
    ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
    ui.input_mut(|input| {
        let mut pressed = |key| input.count_and_consume_key(Modifiers::NONE, key) as f32;
        (pressed(Key::ArrowUp) - pressed(Key::ArrowDown)) * STEP_DB
            + (pressed(Key::PageUp) - pressed(Key::PageDown)) * BIG_STEP_DB
    })
}

/// What the wheel asks of the slider under the pointer, in decibels: a
/// step a notch. On a page that scrolls, only the slider that was last
/// clicked answers the wheel; otherwise scrolling the page past the
/// equalizer would move whichever sliders went under the pointer.
fn wheel(ui: &Ui, response: &Response, place: Place) -> f32 {
    let answers = match place {
        Place::Popup => response.hovered(),
        Place::Page => response.hovered() && response.has_focus(),
    };
    if !answers {
        return 0.0;
    }
    // A touchpad sends many small moves; what has not yet come to a whole
    // notch is carried over to the next frame.
    let carried = response.id.with("wheel");
    let mut notches = ui.data(|data| data.get_temp::<f32>(carried).unwrap_or(0.0));
    ui.input_mut(|input| {
        for event in &input.events {
            if let Event::MouseWheel { unit, delta, .. } = event {
                notches += match unit {
                    MouseWheelUnit::Line => delta.y,
                    MouseWheelUnit::Page => delta.y * 4.0,
                    // Fifty points is about what a wheel's notch scrolls.
                    MouseWheelUnit::Point => delta.y / 50.0,
                };
            }
        }
        // Taken: the page under the slider does not scroll as well.
        input.smooth_scroll_delta = egui::Vec2::ZERO;
    });
    let whole = notches.trunc();
    ui.data_mut(|data| data.insert_temp(carried, notches - whole));
    whole * STEP_DB
}
