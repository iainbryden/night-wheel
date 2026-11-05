//! Appends test results to one local file. Failing to log must never disturb the game.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("WheelDrive").join("load-test.log"))
}

/// Keeps one entry on one line, so a stray value cannot forge another entry.
fn format_line(millis: u128, message: &str) -> String {
    let clean: String = message
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    format!("{millis} {clean}\n")
}

pub fn write(message: &str) {
    let Some(path) = path() else {
        return;
    };
    if let Some(folder) = path.parent() {
        let _ = fs::create_dir_all(folder);
    }
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(format_line(millis, message).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::format_line;

    #[test]
    fn a_line_has_time_and_message() {
        assert_eq!(format_line(12, "plugin loaded"), "12 plugin loaded\n");
    }

    #[test]
    fn line_breaks_cannot_create_extra_entries() {
        assert_eq!(format_line(1, "a\nb\r"), "1 a b \n");
    }
}
