//! Command-line options.

pub const USAGE: &str = "\
wd_diag: read-only wheel diagnostic. Sends nothing to the device.

Usage:
  wd_diag                 Show live controls until Ctrl+C
  wd_diag --list          List game controllers and their controls, then exit
  wd_diag --plain         Print one line per change instead of a live screen
  wd_diag --seconds <n>   Stop after n seconds (1 to 86400)
  wd_diag --help          Show this help";

#[derive(Debug, PartialEq)]
pub enum Mode {
    Help,
    List,
    Live { seconds: Option<u32>, plain: bool },
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut list = false;
        let mut plain = false;
        let mut seconds = None;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(Self { mode: Mode::Help }),
                "--list" => list = true,
                "--plain" => plain = true,
                "--seconds" => {
                    let value = iter.next().ok_or("--seconds needs a number")?;
                    let parsed: u32 = value
                        .parse()
                        .map_err(|_| format!("'{value}' is not a whole number of seconds"))?;
                    if !(1..=86_400).contains(&parsed) {
                        return Err("--seconds must be from 1 to 86400".to_owned());
                    }
                    seconds = Some(parsed);
                }
                other => return Err(format!("unknown option '{other}'")),
            }
        }
        if list && (plain || seconds.is_some()) {
            return Err("--list cannot be combined with --plain or --seconds".to_owned());
        }
        let mode = if list {
            Mode::List
        } else {
            Mode::Live { seconds, plain }
        };
        Ok(Self { mode })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        Options::parse(&owned)
    }

    #[test]
    fn no_arguments_means_live_screen() {
        assert_eq!(
            parse(&[]).unwrap().mode,
            Mode::Live {
                seconds: None,
                plain: false
            }
        );
    }

    #[test]
    fn live_options_combine() {
        assert_eq!(
            parse(&["--plain", "--seconds", "5"]).unwrap().mode,
            Mode::Live {
                seconds: Some(5),
                plain: true
            }
        );
    }

    #[test]
    fn list_and_help_are_recognised() {
        assert_eq!(parse(&["--list"]).unwrap().mode, Mode::List);
        assert_eq!(parse(&["--help"]).unwrap().mode, Mode::Help);
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(parse(&["--seconds"]).is_err());
        assert!(parse(&["--seconds", "soon"]).is_err());
        assert!(parse(&["--seconds", "0"]).is_err());
        assert!(parse(&["--list", "--plain"]).is_err());
        assert!(parse(&["--motor"]).is_err());
    }
}
