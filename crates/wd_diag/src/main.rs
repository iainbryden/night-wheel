//! Standalone read-only wheel diagnostic.
//!
//! Uses Windows Raw Input, which only receives reports. This program never
//! opens a device for writing, so it has no path for motor or LED output.

#[cfg(windows)]
mod device;
#[cfg(windows)]
mod live;
mod options;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match options::Options::parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{}", options::USAGE);
            return std::process::ExitCode::from(2);
        }
    };
    run(options)
}

#[cfg(windows)]
fn run(options: options::Options) -> std::process::ExitCode {
    let result = match options.mode {
        options::Mode::Help => {
            println!("{}", options::USAGE);
            Ok(())
        }
        options::Mode::List => live::list(),
        options::Mode::Live { seconds, plain } => live::run(seconds, plain),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn run(_options: options::Options) -> std::process::ExitCode {
    eprintln!("this diagnostic only runs on Windows");
    std::process::ExitCode::FAILURE
}
