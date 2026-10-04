//! Colours, sizes, fonts and icons.
//!
//! The dark palette and the measurements are the Electron app's (its
//! `tokens.css`): neutral greys, so that artwork carries the colour. Every
//! colour the views use comes from [`Palette`], so a light theme is a second
//! constant, not a hunt through the views.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Vec2,
};

use crate::fonts::Weight;

pub const RADIUS: u8 = 8;
pub const RADIUS_ROW: u8 = 6;

pub const TOP_BAR_HEIGHT: f32 = 52.0;
pub const PLAYER_BAR_HEIGHT: f32 = 80.0;
/// The gap between panels, and between them and the window's edge. The
/// window's own colour shows through it.
pub const GUTTER: i8 = 8;
/// Half a gutter: what each of two neighbouring panels leaves on its side.
pub const HALF_GUTTER: i8 = GUTTER / 2;
/// The sidebar's widths are its card's and the gutters either side of it.
pub const SIDEBAR_WIDTH: f32 = 292.0;
/// Collapsed to a rail of covers: a 72 point card.
pub const SIDEBAR_RAIL_WIDTH: f32 = 84.0;
pub const SIDEBAR_MIN_WIDTH: f32 = 210.0;
pub const SIDEBAR_MAX_WIDTH: f32 = 600.0;
pub const PAGE_PADDING: f32 = 24.0;

pub const WINDOW_SIZE: [f32; 2] = [1240.0, 800.0];
pub const WINDOW_MIN_SIZE: [f32; 2] = [760.0, 520.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Light text on dark, as against dark on light: decides which way a
    /// hover lifts a colour, and how egui's own widgets are drawn.
    pub dark: bool,
    /// Behind everything: the top bar and the gutters between panels.
    pub window: Color32,
    /// The panels: sidebar, page, side panels and the player bar.
    pub panel: Color32,
    /// Chips, fields and tiles.
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    /// Separators and borders.
    pub outline: Color32,
    pub text: Color32,
    pub secondary: Color32,
    /// Disabled controls and hints.
    pub dim: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Text and icons drawn on the accent.
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
    /// Menus, dialogs and toasts.
    pub overlay: Color32,
    pub shadow: Color32,
}

pub const LIGHT: Palette = Palette {
    dark: false,
    window: Color32::from_rgb(0xe4, 0xe7, 0xec),
    panel: Color32::from_rgb(0xff, 0xff, 0xff),
    surface: Color32::from_rgb(0xee, 0xf0, 0xf3),
    surface_hover: Color32::from_rgb(0xe3, 0xe6, 0xeb),
    surface_active: Color32::from_rgb(0xd7, 0xdb, 0xe1),
    outline: Color32::from_rgb(0xdd, 0xe1, 0xe6),
    text: Color32::from_rgb(0x14, 0x17, 0x1a),
    secondary: Color32::from_rgb(0x53, 0x5b, 0x66),
    dim: Color32::from_rgb(0x8b, 0x93, 0x9e),
    accent: Color32::from_rgb(0xe6, 0x00, 0x2e),
    accent_hover: Color32::from_rgb(0xcc, 0x00, 0x29),
    on_accent: Color32::WHITE,
    danger: Color32::from_rgb(0xd6, 0x3b, 0x4c),
    warning: Color32::from_rgb(0xb8, 0x7a, 0x14),
    overlay: Color32::from_rgb(0xff, 0xff, 0xff),
    shadow: Color32::from_black_alpha(50),
};

pub const DARK: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(0x0f, 0x0f, 0x0f),
    panel: Color32::from_rgb(0x18, 0x18, 0x18),
    surface: Color32::from_rgb(0x21, 0x21, 0x21),
    surface_hover: Color32::from_rgb(0x28, 0x28, 0x28),
    // The Electron app's hover and border are white at 14 and 10 parts in
    // a hundred; these are what that comes to on a panel.
    surface_active: Color32::from_rgb(0x38, 0x38, 0x38),
    outline: Color32::from_rgb(0x2f, 0x2f, 0x2f),
    text: Color32::WHITE,
    secondary: Color32::from_rgb(0xaa, 0xaa, 0xaa),
    dim: Color32::from_rgb(0x71, 0x71, 0x71),
    // YouTube's red.
    accent: Color32::from_rgb(0xff, 0x00, 0x33),
    accent_hover: Color32::from_rgb(0xff, 0x33, 0x55),
    on_accent: Color32::WHITE,
    danger: Color32::from_rgb(0xff, 0x4e, 0x45),
    warning: Color32::from_rgb(0xff, 0xa4, 0x2b),
    overlay: Color32::from_rgb(0x28, 0x28, 0x28),
    shadow: Color32::from_black_alpha(140),
};

// Inter's weights are separate families: egui has no notion of weight, so a
// heavier face is a different font.
const MEDIUM: &str = "inter-medium";
const SEMIBOLD: &str = "inter-semibold";
const BOLD: &str = "inter-bold";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Installs fonts, the SVG loader and the base style. Once, at startup.
pub fn install(ctx: &egui::Context, palette: &Palette) {
    egui_extras::install_image_loaders(ctx);
    ctx.set_fonts(fonts(crate::fonts::system()));
    apply(ctx, palette);
}

/// Dresses egui's own widgets in `palette`: at the start, and again when
/// the theme changes.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut style = (*ctx.global_style()).clone();
    apply_to_style(&mut style, palette);
    ctx.set_global_style(style);
}

fn fonts(system: &[crate::fonts::Face]) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    // Whatever egui falls back to (emoji, symbols) stays behind each weight.
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let faces: [(&str, &'static [u8], FontFamily); 4] = [
        (
            "inter-regular",
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            FontFamily::Proportional,
        ),
        (
            MEDIUM,
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
            FontFamily::Name(MEDIUM.into()),
        ),
        (
            SEMIBOLD,
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
            FontFamily::Name(SEMIBOLD.into()),
        ),
        (
            BOLD,
            include_bytes!("../assets/fonts/Inter-Bold.ttf"),
            FontFamily::Name(BOLD.into()),
        ),
    ];
    // Behind those, the system's fonts, for the scripts neither Inter nor
    // egui's own can write.
    for face in system {
        let mut data = FontData::from_static(face.bytes);
        data.index = face.index;
        data.tweak.y_offset_factor = face.drop;
        fonts.font_data.insert(face.name.clone(), Arc::new(data));
    }
    for (name, bytes, family) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        let heavy = matches!(name, SEMIBOLD | BOLD);
        let weight = if heavy { Weight::Bold } else { Weight::Regular };
        let mut stack = vec![name.to_owned()];
        stack.extend(fallbacks.iter().cloned());
        let behind = system.iter().filter(|face| face.weight == weight);
        stack.extend(behind.map(|face| face.name.clone()));
        fonts.families.insert(family, stack);
    }
    fonts
}

/// Applies the palette to egui's own widgets, so text fields, menus and
/// scroll bars agree with the custom views.
fn apply_to_style(style: &mut egui::Style, palette: &Palette) {
    let visuals = &mut style.visuals;
    *visuals = if palette.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    // egui's own buttons, in menus and dialogs, show the hand as the
    // hand-drawn controls do.
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.overlay;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.secondary);
    visuals.hyperlink_color = palette.text;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    visuals.window_corner_radius = CornerRadius::same(RADIUS + 2);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: palette.shadow,
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: palette.shadow,
    };
    let corner = CornerRadius::same(RADIUS_ROW);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = corner;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, palette.text);
        widget.expansion = 0.0;
    }
    visuals.widgets.noninteractive.corner_radius = corner;
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.outline);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.bg_fill = palette.surface;
    visuals.widgets.inactive.weak_bg_fill = palette.surface;
    visuals.widgets.hovered.bg_fill = palette.surface_hover;
    visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
    visuals.widgets.active.bg_fill = palette.surface_active;
    visuals.widgets.active.weak_bg_fill = palette.surface_active;
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent);
    visuals.striped = false;
    // Sliders show how far along they are, with a round handle.
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;

    use egui::TextStyle;
    style.text_styles = [
        (TextStyle::Small, regular(11.5)),
        (TextStyle::Body, regular(14.0)),
        (TextStyle::Button, regular(14.0)),
        (TextStyle::Heading, bold(22.0)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size = Vec2::new(40.0, 28.0);
    // Scroll bars float over the content and stay invisible until used.
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 8.0,
        floating_width: 6.0,
        floating_allocated_width: 0.0,
        handle_min_length: 28.0,
        bar_inner_margin: 3.0,
        bar_outer_margin: 2.0,
        dormant_background_opacity: 0.0,
        dormant_handle_opacity: 0.0,
        active_background_opacity: 0.0,
        active_handle_opacity: 0.55,
        interact_handle_opacity: 0.85,
        foreground_color: true,
        ..egui::style::ScrollStyle::floating()
    };
    // Text in a music client is not a document: dragging across it selects
    // rows, not characters.
    style.interaction.selectable_labels = false;
    style.interaction.tooltip_delay = 0.4;
    style.animation_time = ANIMATION_TIME;
}

/// How long egui's own animations take, while there are any.
pub const ANIMATION_TIME: f32 = 0.12;

macro_rules! icons {
    ($($name:ident => $file:literal),* $(,)?) => {
        /// Every icon the interface draws. The files are Lucide's, with
        /// their colours rewritten to white so [`Icon::image`] can tint them.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($name),*
        }

        impl Icon {
            fn source(self) -> egui::ImageSource<'static> {
                match self {
                    $(Icon::$name => {
                        egui::include_image!(concat!("../assets/icons/", $file, ".svg"))
                    })*
                }
            }
        }
    };
}

icons! {
    Archive => "archive",
    AudioLines => "audio-lines",
    Check => "check",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    CircleAlert => "circle-alert",
    CircleCheck => "circle-check",
    Copy => "copy",
    Disc => "disc-3",
    Ellipsis => "ellipsis",
    Expand => "expand",
    ExternalLink => "external-link",
    Folder => "folder",
    Gift => "gift",
    Headphones => "headphones",
    Heart => "heart",
    HeartFilled => "heart-filled",
    Clock => "clock",
    House => "house",
    LayoutGrid => "layout-grid",
    List => "list",
    ListMusic => "list-music",
    LogOut => "log-out",
    Maximize2 => "maximize-2",
    MicVocal => "mic-vocal",
    Minimize2 => "minimize-2",
    Music => "music",
    PauseFilled => "pause-filled",
    PictureInPicture => "picture-in-picture-2",
    Pin => "pin",
    PlayFilled => "play-filled",
    Plus => "plus",
    Radio => "radio-receiver",
    Repeat => "repeat",
    Repeat1 => "repeat-1",
    RotateCcw => "rotate-ccw",
    Search => "search",
    Settings => "settings",
    Share => "share-2",
    Shrink => "shrink",
    Shuffle => "shuffle",
    SkipBackFilled => "skip-back-filled",
    SlidersVertical => "sliders-vertical",
    SkipForwardFilled => "skip-forward-filled",
    SquareLibrary => "square-library",
    Trash => "trash-2",
    User => "user",
    Video => "video",
    Volume1 => "volume-1",
    Volume2 => "volume-2",
    VolumeX => "volume-x",
    X => "x",
}

impl Icon {
    pub fn image(self, color: Color32, size: f32) -> egui::Image<'static> {
        egui::Image::new(self.source())
            .tint(color)
            .fit_to_exact_size(Vec2::splat(size))
    }

    /// A play triangle's weight sits left of its box, so it is nudged right
    /// to look centred in a round button.
    pub fn optical_offset(self, size: f32) -> Vec2 {
        match self {
            Icon::PlayFilled => Vec2::new(size * 0.03, 0.0),
            _ => Vec2::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the song in the report was called, and a word in each of the
    /// scripts Inter cannot write, beside the font that should write it.
    const WRITTEN: [(&str, &str); 8] = [
        ("YuGothR.ttc", "MOTIVE - 10 MG ︻デ═一"),
        ("YuGothR.ttc", "日本語"),
        ("msyh.ttc", "们这"),
        ("malgun.ttf", "한국어"),
        ("segoeui.ttf", "العربية עברית"),
        ("LeelawUI.ttf", "ไทย"),
        ("Nirmala.ttc", "हिन्दी"),
        ("seguisym.ttf", "★ ♫"),
    ];

    /// A context that has taken the fonts in: they are set on one frame
    /// and there on the next.
    fn wearing(fonts: FontDefinitions) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        let mut drawn = ctx.run_ui(egui::RawInput::default(), |_| {});
        // No screen to hand the font atlas to.
        drawn.textures_delta.clear();
        ctx
    }

    #[cfg(windows)]
    #[test]
    fn what_inter_cannot_write_the_systems_fonts_do_in_every_weight() {
        let Some(windows) = std::env::var_os("SystemRoot") else {
            return;
        };
        let folder = std::path::Path::new(&windows).join("Fonts");
        let system = crate::fonts::load(&folder);
        let ctx = wearing(fonts(&system));
        for (file, text) in WRITTEN {
            // A computer without the font is not one this can be asked of.
            if !folder.join(file).exists() {
                continue;
            }
            for font in [regular(14.0), medium(14.0), semibold(14.0), bold(14.0)] {
                let written = ctx.fonts_mut(|fonts| fonts.has_glyphs(&font, text));
                assert!(written, "{text} cannot be written in {font:?}");
            }
        }
    }

    #[test]
    fn without_the_systems_fonts_inter_still_stands_first() {
        let fonts = fonts(&[]);
        for family in [FontFamily::Proportional, FontFamily::Name(BOLD.into())] {
            let stack = &fonts.families[&family];
            assert!(stack[0].starts_with("inter-"), "{stack:?}");
        }
    }

    #[test]
    fn inter_alone_cannot_write_the_title_that_showed_as_boxes() {
        // The fault as it was: the test above means something only if the
        // bundled fonts really do lack these.
        let ctx = wearing(fonts(&[]));
        let written = ctx.fonts_mut(|fonts| fonts.has_glyphs(&regular(14.0), "︻デ═一"));
        assert!(!written);
    }
}
