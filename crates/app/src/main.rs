//! Youtube Music Spotified, native.
//!
//! The window opens first and everything else follows: the Go core is
//! started after the first frame, and nothing on the UI thread waits on it.

// A release build is a windowed app with no console.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod accounts;
mod actions;
mod app;
mod artistsongs;
mod backend;
mod blocked;
mod changelog;
mod channel;
mod cli;
mod equalizer;
mod fonts;
mod icon;
mod images;
mod logging;
mod migrate;
mod milkdrop;
mod paths;
mod platform;
mod redact;
mod report;
mod resolver;
mod screenshot;
mod session;
mod settings;
mod share;
mod sidecar;
mod signin;
mod single_instance;
mod skin;
mod skins;
mod state;
mod theme;
mod themes;
mod tint;
mod together;
mod update;
mod video;
mod views;

use std::process::ExitCode;
use std::time::Instant;

use eframe::egui;

use crate::app::{App, Launch};
use crate::paths::Paths;
use crate::single_instance::Acquired;

pub(crate) const APP_NAME: &str = "Youtube Music Spotified";
/// Large enough for the taskbar and Alt+Tab; the system scales it down.
const WINDOW_ICON_SIZE: u32 = 256;
/// Distinct from the Electron app's id, so Windows groups the two apart.
const APP_ID: &str = "dev.darksavci.spotified.native";

fn main() -> ExitCode {
    let started = Instant::now();
    // Started as the MilkDrop window: none of the app is wanted, and the
    // profile is the app's that started this, not this process's to open.
    #[cfg(windows)]
    {
        let mut args = std::env::args().skip(1);
        if args.next().as_deref() == Some(milkdrop::child::FLAG) {
            return match milkdrop::child::Args::parse(args) {
                Some(args) => ExitCode::from(milkdrop::child::run(&args) as u8),
                None => ExitCode::from(2),
            };
        }
    }
    let args = match cli::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    match run(args, started) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error}");
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: cli::Args, started: Instant) -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::discover(args.demo, args.profile.as_deref())?;
    let instance = match single_instance::acquire(&paths.instance_lock())? {
        Acquired::First(guard) => guard,
        // The running copy has been asked to show its window.
        Acquired::AlreadyRunning => return Ok(()),
    };
    // A demo without a profile of its own runs in a throwaway one.
    if args.demo && args.profile.is_none() {
        paths.start_clean()?;
    }
    let log_file = logging::init(&paths.logs, started, args.verbose)?;
    log::info!(
        "starting {APP_NAME} {} (log: {})",
        env!("CARGO_PKG_VERSION"),
        log_file.display()
    );

    // Before any window: the taskbar files a window under the id its
    // process had when the window was made.
    platform::identity::claim();

    let args_size = args.size;
    let launch = Launch {
        settings: settings::load(&paths.settings_file()),
        paths,
        instance,
        demo: args.demo,
        screenshot: args.screenshot,
        open: args.open,
        hidden: args.hidden,
        old_profile: args.old_profile,
        started,
    };
    // On Windows the app draws its own title bar unless asked not to.
    let decorated = !cfg!(windows) || launch.settings.system_title_bar;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id(APP_ID)
            .with_icon(egui::IconData {
                rgba: icon::rgba(WINDOW_ICON_SIZE),
                width: WINDOW_ICON_SIZE,
                height: WINDOW_ICON_SIZE,
            })
            .with_decorations(decorated)
            .with_inner_size(args_size.unwrap_or(theme::WINDOW_SIZE))
            .with_min_inner_size(theme::WINDOW_MIN_SIZE),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|context| Ok(Box::new(App::new(context, launch)))),
    )?;
    Ok(())
}
