//! Colours, sizes, fonts and icons.
//!
//! The palette and measurements are Spotifast's dark theme (its
//! `src/theme.rs`). Every colour the views use comes from [`Palette`], so a
//! light theme is a second constant, not a hunt through the views.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Vec2,
};

pub const RADIUS: u8 = 8;
pub const RADIUS_ROW: u8 = 6;

pub const TOP_BAR_HEIGHT: f32 = 48.0;
pub const PLAYER_BAR_HEIGHT: f32 = 80.0;
/// The gap between panels, and between them and the window's edge. The
/// window's own colour shows through it.
pub const GUTTER: i8 = 8;
/// Half a gutter: what each of two neighbouring panels leaves on its side.
pub const HALF_GUTTER: i8 = GUTTER / 2;
pub const SIDEBAR_WIDTH: f32 = 280.0;
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
    window: Color32::from_rgb(0x09, 0x0b, 0x0d),
    panel: Color32::from_rgb(0x15, 0x18, 0x1c),
    surface: Color32::from_rgb(0x1d, 0x21, 0x27),
    surface_hover: Color32::from_rgb(0x26, 0x2b, 0x33),
    surface_active: Color32::from_rgb(0x2f, 0x35, 0x3f),
    outline: Color32::from_rgb(0x2a, 0x30, 0x38),
    text: Color32::from_rgb(0xf2, 0xf4, 0xf6),
    secondary: Color32::from_rgb(0xa9, 0xb1, 0xbc),
    dim: Color32::from_rgb(0x6e, 0x77, 0x84),
    // YouTube's red, as in the Electron app.
    accent: Color32::from_rgb(0xff, 0x00, 0x33),
    accent_hover: Color32::from_rgb(0xff, 0x33, 0x55),
    on_accent: Color32::WHITE,
    danger: Color32::from_rgb(0xf5, 0x71, 0x7f),
    warning: Color32::from_rgb(0xf2, 0xb8, 0x5c),
    overlay: Color32::from_rgb(0x22, 0x27, 0x2e),
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
    ctx.set_fonts(fonts());
    apply(ctx, palette);
}

/// Dresses egui's own widgets in `palette`: at the start, and again when
/// the theme changes.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut style = (*ctx.global_style()).clone();
    apply_to_style(&mut style, palette);
    ctx.set_global_style(style);
}

fn fonts() -> FontDefinitions {
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
    for (name, bytes, family) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        let mut stack = vec![name.to_owned()];
        stack.extend(fallbacks.iter().cloned());
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
    style.animation_time = 0.12;
}

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
    AudioLines => "audio-lines",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    CircleAlert => "circle-alert",
    Expand => "expand",
    ExternalLink => "external-link",
    Folder => "folder",
    Heart => "heart",
    HeartFilled => "heart-filled",
    Clock => "clock",
    House => "house",
    Info => "info",
    LayoutGrid => "layout-grid",
    Library => "library",
    ListMusic => "list-music",
    LogOut => "log-out",
    MicVocal => "mic-vocal",
    Music => "music",
    PanelLeft => "panel-left",
    PauseFilled => "pause-filled",
    Pin => "pin",
    PlayFilled => "play-filled",
    Plus => "plus",
    Repeat => "repeat",
    Repeat1 => "repeat-1",
    Search => "search",
    Settings => "settings",
    Shrink => "shrink",
    Shuffle => "shuffle",
    SkipBackFilled => "skip-back-filled",
    SkipForwardFilled => "skip-forward-filled",
    User => "user",
    Users => "users",
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
