//! Command-line arguments. Three flags do not justify a parser dependency.

use std::path::PathBuf;

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    /// Run on the core's fixture catalogue in a throwaway profile, so nothing
    /// on screen comes from an account.
    pub demo: bool,
    /// Save the window to this PNG once it has settled, then exit.
    pub screenshot: Option<PathBuf>,
    /// Things to do at once, in order: open a page (`album:<id>`,
    /// `artist:<id>`, `playlist:<id>`, `search:<query>`, `settings`), or
    /// start playing `track:<id>`. Given once for each.
    pub open: Vec<String>,
    /// Keep every file under this directory instead of the usual places:
    /// a second, separate installation.
    pub profile: Option<PathBuf>,
    /// Open the window at this size, in points, instead of the usual one.
    pub size: Option<[f32; 2]>,
    /// Start in the tray, without showing the window: how the app is
    /// started at sign-in.
    pub hidden: bool,
    pub verbose: bool,
}

pub const USAGE: &str = "usage: spotified [--demo] [--profile <dir>] [--hidden] [--size <width>x<height>] [--screenshot <file.png>] [--open <kind>:<id>]... [-v]";

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => parsed.demo = true,
            "--hidden" => parsed.hidden = true,
            "-v" | "--verbose" => parsed.verbose = true,
            "--screenshot" => {
                let path = args.next().ok_or("--screenshot needs a file name")?;
                parsed.screenshot = Some(PathBuf::from(path));
            }
            "--profile" => {
                let dir = args.next().ok_or("--profile needs a directory")?;
                parsed.profile = Some(PathBuf::from(dir));
            }
            "--size" => {
                let given = args.next().ok_or("--size needs a size, as 1240x800")?;
                let size = given
                    .split_once('x')
                    .and_then(|(width, height)| Some([width.parse().ok()?, height.parse().ok()?]))
                    .ok_or("--size needs a size, as 1240x800")?;
                parsed.size = Some(size);
            }
            "--open" => {
                parsed.open.push(args.next().ok_or("--open needs a page")?);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(line: &str) -> Result<Args, String> {
        parse(line.split_whitespace().map(str::to_string))
    }

    #[test]
    fn no_arguments_is_a_normal_run() {
        assert_eq!(parse_str(""), Ok(Args::default()));
    }

    #[test]
    fn flags_combine() {
        let args = parse_str("--demo --screenshot out.png -v");
        assert_eq!(
            args,
            Ok(Args {
                demo: true,
                screenshot: Some(PathBuf::from("out.png")),
                open: Vec::new(),
                profile: None,
                size: None,
                hidden: false,
                verbose: true,
            })
        );
    }

    #[test]
    fn open_can_be_given_more_than_once() {
        let args = parse_str("--open settings --open track:abc").map(|args| args.open);
        assert_eq!(
            args,
            Ok(vec!["settings".to_owned(), "track:abc".to_owned()])
        );
    }

    #[test]
    fn a_screenshot_without_a_file_is_refused() {
        assert!(parse_str("--screenshot").is_err());
    }

    #[test]
    fn an_unknown_flag_is_refused() {
        assert!(parse_str("--nope").is_err());
    }
}
