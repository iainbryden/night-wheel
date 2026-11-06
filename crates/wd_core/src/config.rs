//! Versioned configuration: parsing, validation and recoverable saving.
//!
//! The file is plain `key = value` text. Parsing is strict: an unknown,
//! duplicate or missing key rejects the whole file, so a damaged file can
//! never be half-applied. Only settings are stored here, never session state.

use crate::assignment::{Action, Bindings};
use crate::calibration::{CalibrationError, PedalCalibration, SteeringCalibration};
use crate::input::Control;
use crate::wizard::{AxisMap, Calibrated};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Schema version written by this build.
pub const FORMAT_VERSION: u32 = 1;

const BINDING_PREFIX: &str = "binding.";

/// The settings folder's name under `%LOCALAPPDATA%`.
const SETTINGS_FOLDER: &str = "NightWheel";
/// The folder test builds used, and the files worth keeping from it. Its logs stay behind.
const OLD_SETTINGS_FOLDER: &str = "WheelDrive";
const OLD_SETTINGS_FILES: [&str; 3] = ["profile.cfg", "profile.cfg.bak", "steering-speed.txt"];

/// Saved names of functions that no longer exist. Older files may hold them, and that must not make
/// the whole file unusable. Saving drops them.
const RETIRED_BINDINGS: [&str; 1] = ["hover"];

/// The device a configuration was calibrated on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId {
    pub vendor: u32,
    pub product: u32,
}

/// All persisted settings at this stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub device: DeviceId,
    pub axes: AxisMap,
    pub steering: SteeringCalibration,
    pub throttle: PedalCalibration,
    pub brake: PedalCalibration,
    pub bindings: Bindings,
    /// How far boost may raise the car's speed, as a percentage.
    pub boost_percent: u32,
    /// How many degrees of wheel turn reach full lock, side to side.
    pub steering_range: u32,
    /// The steering speed multiplier, once one has been chosen.
    pub steering_speed: Option<f64>,
    /// Whether the game's Settings tab has had its one fill from these values. After that, the tab
    /// holds the player's choices.
    pub settings_tab_filled: bool,
}

/// Boost strength when none has been chosen.
pub const DEFAULT_BOOST_PERCENT: u32 = 25;
/// The strengths a player may choose.
pub const BOOST_PERCENT_RANGE: std::ops::RangeInclusive<u32> = 25..=200;

/// The full travel of the wheel with G HUB at its widest operating range. The steering range is
/// measured against this, because the mod can't read G HUB's setting.
pub const GHUB_NEUTRAL_RANGE: u32 = 900;
/// The steering range for a new calibration, chosen by feel at G HUB sensitivity 50 and range 900.
/// A file saved before ranges existed stays at the full travel, because it was tuned in G HUB.
pub const DEFAULT_STEERING_RANGE: u32 = 360;
/// The steering ranges a player may choose, in degrees.
pub const STEERING_RANGE: std::ops::RangeInclusive<u32> = 180..=GHUB_NEUTRAL_RANGE;
/// The steering speed multipliers a player may choose.
pub const STEERING_SPEED_RANGE: std::ops::RangeInclusive<f64> = 1.0..=10.0;

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
    /// Two of steering, throttle and brake use the same axis.
    SharedAxis,
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
            Self::SharedAxis => write!(f, "steering, throttle and brake need separate axes"),
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
    /// Builds settings from a new calibration. Response choices and assignments carry
    /// over from `previous`, so recalibrating doesn't lose them.
    pub fn from_calibration(
        device: DeviceId,
        calibrated: Calibrated,
        previous: Option<&Self>,
    ) -> Self {
        let mut config = Self {
            device,
            axes: calibrated.axes,
            steering: calibrated.steering,
            throttle: calibrated.throttle,
            brake: calibrated.brake,
            bindings: Bindings::default(),
            boost_percent: DEFAULT_BOOST_PERCENT,
            steering_range: DEFAULT_STEERING_RANGE,
            steering_speed: None,
            settings_tab_filled: false,
        };
        if let Some(previous) = previous {
            config.steering.deadzone = previous.steering.deadzone;
            config.steering.exponent = previous.steering.exponent;
            config.steering.sensitivity = previous.steering.sensitivity;
            config.steering.inverted = previous.steering.inverted;
            for (pedal, old) in [
                (&mut config.throttle, &previous.throttle),
                (&mut config.brake, &previous.brake),
            ] {
                pedal.rest_deadzone = old.rest_deadzone;
                pedal.full_deadzone = old.full_deadzone;
                pedal.exponent = old.exponent;
                pedal.inverted = old.inverted;
            }
            config.bindings = previous.bindings.clone();
            config.boost_percent = previous.boost_percent;
            config.steering_range = previous.steering_range;
            config.steering_speed = previous.steering_speed;
            config.settings_tab_filled = previous.settings_tab_filled;
        }
        config
    }

    /// Checks every value.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let axes = self.axes;
        if axes.steering == axes.throttle
            || axes.steering == axes.brake
            || axes.throttle == axes.brake
        {
            return Err(ConfigError::SharedAxis);
        }
        self.steering.validate()?;
        self.throttle.validate()?;
        self.brake.validate()?;
        if !BOOST_PERCENT_RANGE.contains(&self.boost_percent) {
            return Err(ConfigError::BadValue("boost.percent"));
        }
        if !STEERING_RANGE.contains(&self.steering_range) {
            return Err(ConfigError::BadValue("steering.range"));
        }
        if self
            .steering_speed
            .is_some_and(|speed| !STEERING_SPEED_RANGE.contains(&speed))
        {
            return Err(ConfigError::BadValue("steering.speed"));
        }
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
        let mut config = Self {
            device: DeviceId {
                vendor: fields.take("device.vendor")?,
                product: fields.take("device.product")?,
            },
            axes: AxisMap {
                steering: fields.take("steering.axis")?,
                throttle: fields.take("throttle.axis")?,
                brake: fields.take("brake.axis")?,
            },
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
            bindings: Bindings::default(),
            boost_percent: DEFAULT_BOOST_PERCENT,
            steering_range: GHUB_NEUTRAL_RANGE,
            steering_speed: None,
            settings_tab_filled: false,
        };
        // Files saved before boost existed have no strength, and that is not an error.
        if fields.values.contains_key("boost.percent") {
            config.boost_percent = fields.take("boost.percent")?;
        }
        // Older files have no steering range or speed, and that is not an error.
        if fields.values.contains_key("steering.range") {
            config.steering_range = fields.take("steering.range")?;
        }
        if fields.values.contains_key("steering.speed") {
            config.steering_speed = Some(fields.take("steering.speed")?);
        }
        if fields.values.contains_key("settings.tab_filled") {
            config.settings_tab_filled = fields.take("settings.tab_filled")?;
        }
        for (key, value) in fields.values {
            if key
                .strip_prefix(BINDING_PREFIX)
                .is_some_and(|name| RETIRED_BINDINGS.contains(&name))
            {
                continue;
            }
            let action = key
                .strip_prefix(BINDING_PREFIX)
                .and_then(Action::from_key)
                .ok_or_else(|| ConfigError::UnknownKey(key.clone()))?;
            let control: Control = value
                .parse()
                .map_err(|()| ConfigError::BadValue(action.key()))?;
            config.bindings.assign(action, control);
        }
        config.validate()?;
        Ok(config)
    }

    /// Writes configuration text that [`Config::parse`] reads back exactly.
    pub fn to_text(&self) -> String {
        let s = &self.steering;
        let mut text = format!(
            "version = {FORMAT_VERSION}\n\
             device.vendor = {}\n\
             device.product = {}\n\
             steering.axis = {}\n\
             throttle.axis = {}\n\
             brake.axis = {}\n\
             steering.left = {}\n\
             steering.center = {}\n\
             steering.right = {}\n\
             steering.deadzone = {}\n\
             steering.exponent = {}\n\
             steering.sensitivity = {}\n\
             steering.inverted = {}\n",
            self.device.vendor,
            self.device.product,
            self.axes.steering,
            self.axes.throttle,
            self.axes.brake,
            s.left,
            s.center,
            s.right,
            s.deadzone,
            s.exponent,
            s.sensitivity,
            s.inverted
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
        text.push_str(&format!("boost.percent = {}\n", self.boost_percent));
        text.push_str(&format!("steering.range = {}\n", self.steering_range));
        if let Some(speed) = self.steering_speed {
            text.push_str(&format!("steering.speed = {speed}\n"));
        }
        if self.settings_tab_filled {
            text.push_str("settings.tab_filled = true\n");
        }
        for (action, control) in self.bindings.iter() {
            text.push_str(&format!("{BINDING_PREFIX}{} = {control}\n", action.key()));
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

/// The folder that holds settings and the log.
pub fn settings_folder() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(SETTINGS_FOLDER))
}

/// The main settings file.
pub fn profile_path() -> Option<PathBuf> {
    Some(settings_folder()?.join("profile.cfg"))
}

/// Copies settings from the old folder when the new one has none, and returns how many files it
/// copied. The old folder is left as it is. A failure only means starting without those settings.
pub fn adopt_old_settings() -> usize {
    let Some(new) = settings_folder() else {
        return 0;
    };
    let Some(old) = new.parent().map(|base| base.join(OLD_SETTINGS_FOLDER)) else {
        return 0;
    };
    copy_settings(&old, &new)
}

fn copy_settings(old: &Path, new: &Path) -> usize {
    if new.join(OLD_SETTINGS_FILES[0]).exists() || fs::create_dir_all(new).is_err() {
        return 0;
    }
    OLD_SETTINGS_FILES
        .iter()
        .filter(|name| fs::copy(old.join(name), new.join(name)).is_ok())
        .count()
}

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
    use crate::input::{AxisId, ButtonId};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn axis(usage: u16) -> AxisId {
        AxisId { page: 1, usage }
    }

    fn empty_folders(name: &str) -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("wd_core_adopt_{}_{name}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (old, new) = (base.join("old"), base.join("new"));
        fs::create_dir_all(&old).unwrap();
        (old, new)
    }

    #[test]
    fn old_settings_are_copied_and_the_originals_stay() {
        let (old, new) = empty_folders("copy");
        fs::write(old.join("profile.cfg"), "saved").unwrap();
        fs::write(old.join("load-test.log"), "log").unwrap();
        assert_eq!(copy_settings(&old, &new), 1);
        assert_eq!(
            fs::read_to_string(new.join("profile.cfg")).unwrap(),
            "saved"
        );
        assert!(old.join("profile.cfg").exists());
        assert!(!new.join("load-test.log").exists());
    }

    #[test]
    fn old_settings_never_replace_new_ones() {
        let (old, new) = empty_folders("keep");
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("profile.cfg"), "old").unwrap();
        fs::write(new.join("profile.cfg"), "new").unwrap();
        assert_eq!(copy_settings(&old, &new), 0);
        assert_eq!(fs::read_to_string(new.join("profile.cfg")).unwrap(), "new");
    }

    #[test]
    fn steering_choices_survive_saving_and_older_files_get_neutral_values() {
        let mut config = sample();
        config.steering_range = 270;
        config.steering_speed = Some(4.0);
        let parsed = Config::parse(&config.to_text()).unwrap();
        assert_eq!(parsed.steering_range, 270);
        assert_eq!(parsed.steering_speed, Some(4.0));
        let older: String = config
            .to_text()
            .lines()
            .filter(|line| {
                !line.starts_with("steering.range") && !line.starts_with("steering.speed")
            })
            .map(|line| format!("{line}\n"))
            .collect();
        let parsed = Config::parse(&older).unwrap();
        assert_eq!(parsed.steering_range, GHUB_NEUTRAL_RANGE);
        assert_eq!(parsed.steering_speed, None);
    }

    #[test]
    fn the_tab_fill_is_remembered_and_older_files_have_not_had_it() {
        let mut config = sample();
        assert!(
            !Config::parse(&config.to_text())
                .unwrap()
                .settings_tab_filled
        );
        config.settings_tab_filled = true;
        assert!(
            Config::parse(&config.to_text())
                .unwrap()
                .settings_tab_filled
        );
    }

    #[test]
    fn steering_choices_outside_their_limits_are_refused() {
        let mut config = sample();
        config.steering_range = 90;
        assert!(config.validate().is_err());
        config = sample();
        config.steering_speed = Some(11.0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn a_retired_binding_is_ignored_and_dropped_on_saving() {
        let older = format!("{}binding.hover = button:0009:0008\n", sample().to_text());
        let parsed = Config::parse(&older).unwrap();
        assert_eq!(parsed.bindings, sample().bindings);
        assert!(!parsed.to_text().contains("hover"));
    }

    #[test]
    fn a_file_without_a_boost_strength_gets_the_default() {
        let mut config = sample();
        config.boost_percent = 60;
        let saved = config.to_text();
        assert_eq!(Config::parse(&saved).unwrap().boost_percent, 60);
        let older: String = saved
            .lines()
            .filter(|line| !line.starts_with("boost.percent"))
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(
            Config::parse(&older).unwrap().boost_percent,
            DEFAULT_BOOST_PERCENT
        );
    }

    #[test]
    fn a_boost_strength_out_of_range_is_refused() {
        for bad in [0, 24, 201] {
            let mut config = sample();
            config.boost_percent = bad;
            assert!(config.validate().is_err(), "{bad}");
        }
        for good in [25, 100, 200] {
            let mut config = sample();
            config.boost_percent = good;
            assert!(config.validate().is_ok(), "{good}");
        }
    }

    #[test]
    fn bindings_round_trip() {
        let mut config = sample();
        config.bindings.assign(
            Action::ShiftUp,
            Control::Button(ButtonId { page: 9, usage: 5 }),
        );
        config.bindings.assign(Action::Horn, Control::Hat(2));
        assert_eq!(Config::parse(&config.to_text()), Ok(config));
    }

    #[test]
    fn unknown_binding_is_rejected() {
        let text = format!("{}binding.warp = hat:1\n", sample().to_text());
        assert_eq!(
            Config::parse(&text),
            Err(ConfigError::UnknownKey("binding.warp".into()))
        );
    }

    #[test]
    fn malformed_binding_is_rejected() {
        let text = format!("{}binding.horn = nonsense\n", sample().to_text());
        assert_eq!(Config::parse(&text), Err(ConfigError::BadValue("horn")));
    }

    #[test]
    fn shared_axis_is_rejected() {
        let mut config = sample();
        config.axes.brake = config.axes.throttle;
        assert_eq!(config.validate(), Err(ConfigError::SharedAxis));
    }

    #[test]
    fn recalibrating_keeps_response_choices_and_assignments() {
        let mut previous = sample();
        previous.bindings.assign(Action::Horn, Control::Hat(1));
        let calibrated = Calibrated {
            axes: AxisMap {
                steering: axis(4),
                throttle: axis(5),
                brake: axis(6),
            },
            steering: SteeringCalibration {
                left: 0,
                center: 50,
                right: 100,
                deadzone: 0.0,
                exponent: 1.0,
                sensitivity: 1.0,
                inverted: false,
            },
            throttle: PedalCalibration {
                rest: 0,
                full: 10,
                rest_deadzone: 0.0,
                full_deadzone: 0.0,
                exponent: 1.0,
                inverted: false,
            },
            brake: PedalCalibration {
                rest: 0,
                full: 20,
                rest_deadzone: 0.0,
                full_deadzone: 0.0,
                exponent: 1.0,
                inverted: false,
            },
        };
        let config = Config::from_calibration(previous.device, calibrated, Some(&previous));
        assert_eq!(config.steering.left, 0);
        assert_eq!(config.steering.deadzone, previous.steering.deadzone);
        assert_eq!(config.steering.inverted, previous.steering.inverted);
        assert_eq!(config.brake.full, 20);
        assert_eq!(config.brake.exponent, previous.brake.exponent);
        assert_eq!(config.bindings, previous.bindings);
        assert_eq!(config.axes.steering, axis(4));
    }

    fn sample() -> Config {
        Config {
            device: DeviceId {
                vendor: 1,
                product: 2,
            },
            axes: AxisMap {
                steering: axis(0x30),
                throttle: axis(0x31),
                brake: axis(0x35),
            },
            bindings: Bindings::default(),
            boost_percent: DEFAULT_BOOST_PERCENT,
            steering_range: GHUB_NEUTRAL_RANGE,
            steering_speed: None,
            settings_tab_filled: false,
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
