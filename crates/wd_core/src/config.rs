//! Versioned configuration: parsing, validation and recoverable saving.
//!
//! The file is plain `key = value` text. Parsing is strict: an unknown,
//! duplicate or missing key rejects the whole file, so a damaged file can
//! never be half-applied. Only settings are stored here, never session state.

use crate::calibration::{CalibrationError, PedalCalibration, SteeringCalibration};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Schema version written by this build.
pub const FORMAT_VERSION: u32 = 1;

/// All persisted settings at this stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    pub steering: SteeringCalibration,
    pub throttle: PedalCalibration,
    pub brake: PedalCalibration,
}

/// Why a configuration could not be parsed or accepted.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// A line is not `key = value`.
    MalformedLine(usize),
    /// A key appears more than once.
    DuplicateKey(String),
    /// A key is not part of this schema.
    UnknownKey(String),
    /// A required key is absent.
    MissingKey(&'static str),
    /// A value cannot be read as the required type.
    BadValue(&'static str),
    /// The file was written by an unsupported schema version.
    UnsupportedVersion(u32),
    /// The values parsed but failed validation.
    Invalid(CalibrationError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedLine(line) => write!(f, "line {line} is not 'key = value'"),
            Self::DuplicateKey(key) => write!(f, "duplicate setting '{key}'"),
            Self::UnknownKey(key) => write!(f, "unknown setting '{key}'"),
            Self::MissingKey(key) => write!(f, "missing setting '{key}'"),
            Self::BadValue(key) => write!(f, "setting '{key}' has an invalid value"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported settings version {version}")
            }
            Self::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<CalibrationError> for ConfigError {
    fn from(error: CalibrationError) -> Self {
        Self::Invalid(error)
    }
}

struct Fields {
    values: BTreeMap<String, String>,
}

impl Fields {
    fn take<T: FromStr>(&mut self, key: &'static str) -> Result<T, ConfigError> {
        let text = self
            .values
            .remove(key)
            .ok_or(ConfigError::MissingKey(key))?;
        text.parse().map_err(|_| ConfigError::BadValue(key))
    }

    fn take_pedal(&mut self, keys: [&'static str; 6]) -> Result<PedalCalibration, ConfigError> {
        Ok(PedalCalibration {
            rest: self.take(keys[0])?,
            full: self.take(keys[1])?,
            rest_deadzone: self.take(keys[2])?,
            full_deadzone: self.take(keys[3])?,
            exponent: self.take(keys[4])?,
            inverted: self.take(keys[5])?,
        })
    }
}

impl Config {
    /// Checks every value.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.steering.validate()?;
        self.throttle.validate()?;
        self.brake.validate()?;
        Ok(())
    }

    /// Parses and validates configuration text.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let mut values = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or(ConfigError::MalformedLine(index + 1))?;
            let key = key.trim();
            if key.is_empty() {
                return Err(ConfigError::MalformedLine(index + 1));
            }
            if values
                .insert(key.to_owned(), value.trim().to_owned())
                .is_some()
            {
                return Err(ConfigError::DuplicateKey(key.to_owned()));
            }
        }

        let mut fields = Fields { values };
        let version: u32 = fields.take("version")?;
        if version != FORMAT_VERSION {
            return Err(ConfigError::UnsupportedVersion(version));
        }
        let config = Self {
            steering: SteeringCalibration {
                left: fields.take("steering.left")?,
                center: fields.take("steering.center")?,
                right: fields.take("steering.right")?,
                deadzone: fields.take("steering.deadzone")?,
                exponent: fields.take("steering.exponent")?,
                sensitivity: fields.take("steering.sensitivity")?,
                inverted: fields.take("steering.inverted")?,
            },
            throttle: fields.take_pedal([
                "throttle.rest",
                "throttle.full",
                "throttle.rest_deadzone",
                "throttle.full_deadzone",
                "throttle.exponent",
                "throttle.inverted",
            ])?,
            brake: fields.take_pedal([
                "brake.rest",
                "brake.full",
                "brake.rest_deadzone",
                "brake.full_deadzone",
                "brake.exponent",
                "brake.inverted",
            ])?,
        };
        if let Some(key) = fields.values.into_keys().next() {
            return Err(ConfigError::UnknownKey(key));
        }
        config.validate()?;
        Ok(config)
    }

    /// Writes configuration text that [`Config::parse`] reads back exactly.
    pub fn to_text(&self) -> String {
        let s = &self.steering;
        let mut text = format!(
            "version = {FORMAT_VERSION}\n\
             steering.left = {}\n\
             steering.center = {}\n\
             steering.right = {}\n\
             steering.deadzone = {}\n\
             steering.exponent = {}\n\
             steering.sensitivity = {}\n\
             steering.inverted = {}\n",
            s.left, s.center, s.right, s.deadzone, s.exponent, s.sensitivity, s.inverted
        );
        for (name, p) in [("throttle", &self.throttle), ("brake", &self.brake)] {
            text.push_str(&format!(
                "{name}.rest = {}\n\
                 {name}.full = {}\n\
                 {name}.rest_deadzone = {}\n\
                 {name}.full_deadzone = {}\n\
                 {name}.exponent = {}\n\
                 {name}.inverted = {}\n",
                p.rest, p.full, p.rest_deadzone, p.full_deadzone, p.exponent, p.inverted
            ));
        }
        text
    }
}

/// Where a loaded configuration came from.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadSource {
    Primary,
    /// The main file was unusable, so the backup was used. The main file is left in place.
    Backup {
        primary_problem: String,
    },
}

/// Result of a successful load.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    pub source: LoadSource,
}

/// Why neither the main file nor the backup could be used.
#[derive(Debug)]
pub struct LoadError {
    pub primary_problem: String,
    pub backup_problem: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "settings could not be loaded ({}); backup could not be loaded ({})",
            self.primary_problem, self.backup_problem
        )
    }
}

impl std::error::Error for LoadError {}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

/// Path of the backup kept beside `path`.
pub fn backup_path(path: &Path) -> PathBuf {
    with_suffix(path, ".bak")
}

fn read_config(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Config::parse(&text).map_err(|error| error.to_string())
}

fn write_synced(path: &Path, text: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// Saves `config` by writing a new file and renaming it over the old one.
///
/// A currently valid main file becomes the backup first. A damaged main file
/// never replaces the backup, so the last good settings stay recoverable.
pub fn save(path: &Path, config: &Config) -> Result<(), SaveError> {
    config.validate().map_err(SaveError::Invalid)?;
    if let Ok(previous) = read_config(path) {
        write_synced(&backup_path(path), &previous.to_text()).map_err(SaveError::Io)?;
    }
    let temporary = with_suffix(path, ".tmp");
    write_synced(&temporary, &config.to_text()).map_err(SaveError::Io)?;
    fs::rename(&temporary, path).map_err(SaveError::Io)
}

/// Why a configuration was not saved.
#[derive(Debug)]
pub enum SaveError {
    Invalid(ConfigError),
    Io(io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => write!(f, "settings not saved: {error}"),
            Self::Io(error) => write!(f, "settings not saved: {error}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Loads the main file, falling back to the backup when it is unusable.
pub fn load(path: &Path) -> Result<Loaded, LoadError> {
    let primary_problem = match read_config(path) {
        Ok(config) => {
            return Ok(Loaded {
                config,
                source: LoadSource::Primary,
            });
        }
        Err(problem) => problem,
    };
    match read_config(&backup_path(path)) {
        Ok(config) => Ok(Loaded {
            config,
            source: LoadSource::Backup { primary_problem },
        }),
        Err(backup_problem) => Err(LoadError {
            primary_problem,
            backup_problem,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn sample() -> Config {
        Config {
            steering: SteeringCalibration {
                left: -100,
                center: 3,
                right: 100,
                deadzone: 0.125,
                exponent: 1.5,
                sensitivity: 0.75,
                inverted: true,
            },
            throttle: PedalCalibration {
                rest: 900,
                full: 10,
                rest_deadzone: 0.1,
                full_deadzone: 0.2,
                exponent: 1.0,
                inverted: false,
            },
            brake: PedalCalibration {
                rest: 5,
                full: 800,
                rest_deadzone: 0.0,
                full_deadzone: 0.05,
                exponent: 2.0,
                inverted: true,
            },
        }
    }

    /// A unique empty directory, removed when the test ends.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("wd_core_test_{}_{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self) -> PathBuf {
            self.0.join("settings.cfg")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn text_round_trips_exactly() {
        let config = sample();
        assert_eq!(Config::parse(&config.to_text()), Ok(config));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let text = format!("# note\n\n{}", sample().to_text());
        assert_eq!(Config::parse(&text), Ok(sample()));
    }

    #[test]
    fn malformed_line_reports_its_number() {
        let text = format!("{}nonsense\n", sample().to_text());
        let line = sample().to_text().lines().count() + 1;
        assert_eq!(Config::parse(&text), Err(ConfigError::MalformedLine(line)));
    }

    #[test]
    fn duplicate_key_is_rejected() {
        let text = format!("{}steering.left = 1\n", sample().to_text());
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::DuplicateKey("steering.left".into()))
        );
    }

    #[test]
    fn unknown_key_is_rejected() {
        let text = format!("{}extra = 1\n", sample().to_text());
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::UnknownKey("extra".into()))
        );
    }

    #[test]
    fn missing_key_is_rejected() {
        let text = sample().to_text().replace("brake.exponent = 2\n", "");
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::MissingKey("brake.exponent"))
        );
    }

    #[test]
    fn wrong_type_is_rejected() {
        let text = sample()
            .to_text()
            .replace("steering.left = -100", "steering.left = lots");
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::BadValue("steering.left"))
        );
    }

    #[test]
    fn future_version_is_rejected() {
        let text = sample().to_text().replace("version = 1", "version = 2");
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn non_finite_numbers_are_rejected() {
        for bad in ["NaN", "inf", "-inf"] {
            let text = sample().to_text().replace(
                "steering.exponent = 1.5",
                &format!("steering.exponent = {bad}"),
            );
            assert!(matches!(
                Config::parse(&text),
                Err(ConfigError::Invalid(CalibrationError::NotFinite(_)))
            ));
        }
    }

    #[test]
    fn zero_span_calibration_is_rejected() {
        let text = sample()
            .to_text()
            .replace("throttle.full = 10", "throttle.full = 900");
        assert!(matches!(
            Config::parse(&text),
            Err(ConfigError::Invalid(CalibrationError::ZeroSpan(_)))
        ));
    }

    #[test]
    fn save_then_load_returns_the_same_settings() {
        let dir = TestDir::new();
        save(&dir.file(), &sample()).unwrap();
        let loaded = load(&dir.file()).unwrap();
        assert_eq!(loaded.config, sample());
        assert_eq!(loaded.source, LoadSource::Primary);
    }

    #[test]
    fn save_leaves_no_temporary_file() {
        let dir = TestDir::new();
        save(&dir.file(), &sample()).unwrap();
        assert!(!with_suffix(&dir.file(), ".tmp").exists());
    }

    #[test]
    fn invalid_settings_are_not_saved_and_change_nothing() {
        let dir = TestDir::new();
        save(&dir.file(), &sample()).unwrap();
        let mut bad = sample();
        bad.brake.exponent = f64::NAN;
        assert!(matches!(
            save(&dir.file(), &bad),
            Err(SaveError::Invalid(_))
        ));
        assert_eq!(load(&dir.file()).unwrap().config, sample());
    }

    #[test]
    fn second_save_backs_up_the_previous_settings() {
        let dir = TestDir::new();
        let mut newer = sample();
        newer.steering.left = -200;
        save(&dir.file(), &sample()).unwrap();
        save(&dir.file(), &newer).unwrap();
        assert_eq!(load(&dir.file()).unwrap().config, newer);
        assert_eq!(read_config(&backup_path(&dir.file())).unwrap(), sample());
    }

    #[test]
    fn damaged_file_recovers_from_backup_and_is_kept() {
        let dir = TestDir::new();
        save(&dir.file(), &sample()).unwrap();
        save(&dir.file(), &sample()).unwrap();
        fs::write(dir.file(), "version = broken").unwrap();
        let loaded = load(&dir.file()).unwrap();
        assert_eq!(loaded.config, sample());
        assert!(matches!(loaded.source, LoadSource::Backup { .. }));
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), "version = broken");
    }

    #[test]
    fn saving_over_a_damaged_file_keeps_the_good_backup() {
        let dir = TestDir::new();
        let mut newer = sample();
        newer.steering.left = -200;
        save(&dir.file(), &sample()).unwrap();
        save(&dir.file(), &sample()).unwrap();
        fs::write(dir.file(), "garbage").unwrap();
        save(&dir.file(), &newer).unwrap();
        assert_eq!(read_config(&backup_path(&dir.file())).unwrap(), sample());
        assert_eq!(load(&dir.file()).unwrap().config, newer);
    }

    #[test]
    fn missing_files_report_both_problems() {
        let dir = TestDir::new();
        let error = load(&dir.file()).unwrap_err();
        assert!(!error.primary_problem.is_empty());
        assert!(!error.backup_problem.is_empty());
    }
}
