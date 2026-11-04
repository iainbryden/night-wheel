//! The app's screens. Rules live in `wd_core`; this file only displays them and collects input.

use eframe::egui::{self, Color32, ProgressBar, RichText, Slider};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wd_core::assignment::Action;
use wd_core::calibration::PedalCalibration;
use wd_core::config::{self, Config, DeviceId, LoadSource};
use wd_core::input::{AxisId, Control, Sample};
use wd_core::listen::{ButtonListener, LISTEN_TIMEOUT_MS, ListenOutcome};
use wd_core::wizard::{Step, Wizard};
use wd_device::monitor::{DeviceView, Monitor};

const WARNING: Color32 = Color32::from_rgb(220, 150, 40);
const ERROR: Color32 = Color32::from_rgb(220, 80, 70);
const ACTIVE: Color32 = Color32::from_rgb(80, 180, 100);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Device,
    Calibrate,
    Response,
    Assign,
    Later,
}

struct PendingAssignment {
    action: Action,
    control: Control,
    conflicts: Vec<Action>,
}

/// All app state.
pub struct SetupApp {
    monitor: Monitor,
    started: Instant,
    screen: Screen,
    settings_path: Option<PathBuf>,
    config: Option<Config>,
    unsaved: bool,
    status: Option<(String, Color32)>,
    wizard: Option<Wizard>,
    wizard_problem: Option<String>,
    listening: Option<(Action, ButtonListener)>,
    pending: Option<PendingAssignment>,
}

/// Debug builds can open on a chosen screen, so each one can be checked without clicking.
#[cfg(debug_assertions)]
fn initial_screen() -> Screen {
    match std::env::var("WD_SETUP_SCREEN").as_deref() {
        Ok("calibrate") => Screen::Calibrate,
        Ok("response") => Screen::Response,
        Ok("assign") => Screen::Assign,
        Ok("later") => Screen::Later,
        _ => Screen::Device,
    }
}

#[cfg(not(debug_assertions))]
fn initial_screen() -> Screen {
    Screen::Device
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("WheelDrive").join("profile.cfg"))
}

impl SetupApp {
    /// Starts reading devices and loads saved settings.
    pub fn new(context: egui::Context) -> Self {
        let monitor = Monitor::start(move || context.request_repaint());
        let settings_path = settings_path();
        let mut config = None;
        let mut status = None;
        match &settings_path {
            None => {
                status = Some((
                    "Can't find a folder for settings, so saving is unavailable.".to_owned(),
                    ERROR,
                ));
            }
            Some(path) if path.exists() || config::backup_path(path).exists() => {
                match config::load(path) {
                    Ok(loaded) => {
                        if let LoadSource::Backup { primary_problem } = &loaded.source {
                            status = Some((
                                format!(
                                    "Your settings file couldn't be used ({primary_problem}), so the backup was loaded."
                                ),
                                WARNING,
                            ));
                        }
                        config = Some(loaded.config);
                    }
                    Err(error) => status = Some((error.to_string(), ERROR)),
                }
            }
            Some(_) => {}
        }
        Self {
            monitor,
            started: Instant::now(),
            screen: initial_screen(),
            settings_path,
            config,
            unsaved: false,
            status,
            wizard: None,
            wizard_problem: None,
            listening: None,
            pending: None,
        }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn save(&mut self) {
        let (Some(path), Some(config)) = (&self.settings_path, &self.config) else {
            return;
        };
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(|error| error.to_string())
            .and_then(|()| config::save(path, config).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {
                self.unsaved = false;
                self.status = Some(("Settings saved.".to_owned(), ACTIVE));
            }
            Err(error) => self.status = Some((error, ERROR)),
        }
    }

    fn device_panel(&mut self, ui: &mut egui::Ui, device: Option<&DeviceView>) {
        ui.heading("Device");
        ui.label("Read-only: this app sends nothing to the wheel, so nothing here can move it.");
        ui.add_space(8.0);
        let Some(device) = device else {
            ui.colored_label(WARNING, "No wheel or game controller is connected.");
            return;
        };
        ui.label(RichText::new(&device.label).strong());
        ui.label(format!("Connected, {} reports read", device.reports));
        if let Some(config) = &self.config
            && (config.device.vendor, config.device.product) != (device.vendor, device.product)
        {
            ui.colored_label(
                WARNING,
                "These settings were calibrated on a different device.",
            );
        }
        ui.add_space(8.0);
        ui.label("Move a control to see which entry responds.");
        egui::Grid::new("axes").num_columns(3).show(ui, |ui| {
            for axis in &device.sample.axes {
                ui.label(self.axis_name(axis.id));
                ui.add(
                    ProgressBar::new(axis.fraction().unwrap_or(0.0) as f32).desired_width(280.0),
                );
                ui.label(axis.value.to_string());
                ui.end_row();
            }
        });
        if device.sample.axes.is_empty() {
            ui.label("No readings yet. Move the wheel to wake it up.");
        }
        ui.add_space(8.0);
        let held: Vec<String> = device.sample.held().iter().map(Control::label).collect();
        ui.label(format!(
            "Held: {}",
            if held.is_empty() {
                "nothing".to_owned()
            } else {
                held.join(", ")
            }
        ));
    }

    fn axis_name(&self, id: AxisId) -> String {
        if let Some(config) = &self.config {
            let axes = config.axes;
            if id == axes.steering {
                return format!("Steering ({id})");
            }
            if id == axes.throttle {
                return format!("Throttle ({id})");
            }
            if id == axes.brake {
                return format!("Brake ({id})");
            }
        }
        format!("Axis {id}")
    }

    fn calibrate_panel(&mut self, ui: &mut egui::Ui, device: Option<&DeviceView>) {
        ui.heading("Calibrate");
        let Some(wizard) = &mut self.wizard else {
            ui.label("This finds your wheel and pedals by asking you to move them, then records their range.");
            if self.config.is_some() {
                ui.label("You're already calibrated. Running this again keeps your response settings and assignments.");
            }
            ui.add_space(8.0);
            if ui
                .add_enabled(device.is_some(), egui::Button::new("Start calibration"))
                .clicked()
            {
                self.wizard = Some(Wizard::new());
                self.wizard_problem = None;
            }
            if device.is_none() {
                ui.colored_label(WARNING, "Connect a wheel first.");
            }
            return;
        };

        let step = wizard.step();
        ui.add_space(8.0);
        ui.label(RichText::new(step.instruction()).size(18.0));
        ui.add_space(8.0);

        let Some(device) = device else {
            ui.colored_label(
                WARNING,
                "The wheel is disconnected. Reconnect it to continue.",
            );
            if ui.button("Cancel").clicked() {
                self.wizard = None;
            }
            return;
        };
        wizard.observe(&device.sample);

        if let Some(id) = wizard.current_axis() {
            if let Some(axis) = device.sample.axis(id) {
                ui.horizontal(|ui| {
                    ui.label(format!("Axis {id}"));
                    ui.add(
                        ProgressBar::new(axis.fraction().unwrap_or(0.0) as f32)
                            .desired_width(280.0),
                    );
                    ui.label(axis.value.to_string());
                });
            }
        } else if matches!(step, Step::ThrottleFull | Step::BrakeFull) {
            ui.label("Waiting for the pedal to move...");
        }

        if let Some(problem) = &self.wizard_problem {
            ui.colored_label(ERROR, problem);
        }
        ui.add_space(8.0);

        let mut finish = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            if wizard.needs_capture() && ui.button("Capture").clicked() {
                self.wizard_problem = wizard
                    .capture(&device.sample)
                    .err()
                    .map(|error| error.to_string());
            }
            if wizard.step() == Step::Done && ui.button("Use this calibration").clicked() {
                finish = true;
            }
            if wizard.step() != Step::FindSteering && ui.button("Back").clicked() {
                wizard.back();
                self.wizard_problem = None;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });

        if finish && let Some(result) = wizard.result() {
            let id = DeviceId {
                vendor: device.vendor,
                product: device.product,
            };
            self.config = Some(Config::from_calibration(id, result, self.config.as_ref()));
            self.unsaved = true;
            self.wizard = None;
            self.save();
            self.screen = Screen::Response;
        } else if cancel {
            self.wizard = None;
            self.wizard_problem = None;
        }
    }

    fn response_panel(&mut self, ui: &mut egui::Ui, device: Option<&DeviceView>) {
        ui.heading("Steering and pedals");
        let sample = device
            .map(|device| device.sample.clone())
            .unwrap_or_default();
        let Some(config) = &mut self.config else {
            ui.label("Calibrate first to adjust these.");
            return;
        };
        ui.label("Bars show your live input after these settings. They don't reach the game yet.");
        ui.add_space(8.0);

        let before = config.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(RichText::new("Steering").strong());
            if let Some(axis) = sample.axis(config.axes.steering)
                && let Ok(raw) = i32::try_from(axis.value)
            {
                let output = config.steering.normalize(raw);
                ui.add(
                    ProgressBar::new(((output + 1.0) / 2.0) as f32)
                        .desired_width(320.0)
                        .text(format!("{output:+.2}")),
                );
            }
            let steering = &mut config.steering;
            ui.add(Slider::new(&mut steering.sensitivity, 0.1..=3.0).text("Sensitivity"));
            ui.add(Slider::new(&mut steering.deadzone, 0.0..=0.5).text("Centre deadzone"));
            ui.add(Slider::new(&mut steering.exponent, 0.2..=5.0).text("Response curve"));
            ui.checkbox(&mut steering.inverted, "Invert");
            ui.add_space(12.0);
            pedal_controls(
                ui,
                "Throttle",
                &mut config.throttle,
                config.axes.throttle,
                &sample,
            );
            ui.add_space(12.0);
            pedal_controls(ui, "Brake", &mut config.brake, config.axes.brake, &sample);
            ui.add_space(12.0);
            if ui.button("Remove all shaping").clicked() {
                steering_reset(config);
            }
        });
        if *config != before {
            self.unsaved = true;
        }
    }

    fn update_listen(&mut self, sample: Option<&Sample>) {
        let now = self.now_ms();
        let Some((action, listener)) = &mut self.listening else {
            return;
        };
        let action = *action;
        match listener.update(now, sample) {
            ListenOutcome::Waiting => {}
            ListenOutcome::TimedOut => {
                self.listening = None;
                self.status = Some(("Listening timed out. Nothing changed.".to_owned(), WARNING));
            }
            ListenOutcome::Detected(control) => {
                self.listening = None;
                let conflicts = self
                    .config
                    .as_ref()
                    .map(|config| config.bindings.conflicts(action, control))
                    .unwrap_or_default();
                self.pending = Some(PendingAssignment {
                    action,
                    control,
                    conflicts,
                });
            }
        }
    }

    fn assign_panel(&mut self, ui: &mut egui::Ui, device: Option<&DeviceView>) {
        ui.heading("Assignments");
        if self.config.is_none() {
            ui.label("Calibrate first to assign controls.");
            return;
        }
        ui.label("Choose Listen, then press the control. These are saved for later: nothing reaches the game yet, and each action still has to be proven in the game.");
        ui.add_space(8.0);
        let sample = device.map(|device| &device.sample);
        self.update_listen(sample);

        if let Some(action) = self.listening.as_ref().map(|(action, _)| *action) {
            ui.horizontal(|ui| {
                ui.label(format!("Press a control for: {}", action.label()));
                if ui.button("Cancel").clicked() {
                    self.listening = None;
                }
            });
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        if let Some(pending) = &self.pending {
            ui.label(format!(
                "Assign {} to {}?",
                pending.control.label(),
                pending.action.label()
            ));
            if !pending.conflicts.is_empty() {
                let names: Vec<&str> = pending.conflicts.iter().map(|a| a.label()).collect();
                ui.colored_label(
                    WARNING,
                    format!(
                        "It's already used by {}. Confirming makes one control do both.",
                        names.join(", ")
                    ),
                );
            }
            let mut decision = None;
            ui.horizontal(|ui| {
                if ui.button("Confirm").clicked() {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
            match decision {
                Some(true) => {
                    if let (Some(config), Some(pending)) = (&mut self.config, self.pending.take()) {
                        config.bindings.assign(pending.action, pending.control);
                        self.unsaved = true;
                    }
                }
                Some(false) => self.pending = None,
                None => {}
            }
            ui.add_space(8.0);
        }

        let busy = self.listening.is_some() || self.pending.is_some();
        let now = self.now_ms();
        let mut listen_for = None;
        let mut clear = None;
        let Some(config) = &self.config else {
            return;
        };
        let active = sample
            .map(|sample| config.bindings.active(sample))
            .unwrap_or_default();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("assignments")
                .num_columns(4)
                .show(ui, |ui| {
                    for action in Action::ALL {
                        ui.label(action.label());
                        let bound = config.bindings.get(action);
                        ui.label(bound.map_or_else(|| "Not assigned".to_owned(), |c| c.label()));
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!busy, egui::Button::new("Listen")).clicked() {
                                listen_for = Some(action);
                            }
                            if ui
                                .add_enabled(!busy && bound.is_some(), egui::Button::new("Clear"))
                                .clicked()
                            {
                                clear = Some(action);
                            }
                        });
                        if active.contains(&action) {
                            ui.colored_label(ACTIVE, "Held");
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
        });
        if let Some(action) = listen_for {
            self.listening = Some((action, ButtonListener::new(now, sample, LISTEN_TIMEOUT_MS)));
        }
        if let Some(action) = clear
            && let Some(config) = &mut self.config
        {
            config.bindings.clear(action);
            self.unsaved = true;
        }
    }

    fn later_panel(ui: &mut egui::Ui) {
        ui.heading("Not available yet");
        ui.label("These need more testing before they can be offered:");
        ui.add_space(8.0);
        for item in [
            "Force feedback, and a test with a Stop button",
            "Wheel rotation",
            "Wheel LEDs",
            "Auto/Manual transmission",
            "Boost",
            "Multiple named profiles and per-vehicle settings",
            "Using any of this in the game",
        ] {
            ui.label(format!("• {item}"));
        }
    }
}

fn pedal_controls(
    ui: &mut egui::Ui,
    name: &str,
    pedal: &mut PedalCalibration,
    axis: AxisId,
    sample: &Sample,
) {
    ui.label(RichText::new(name).strong());
    if let Some(state) = sample.axis(axis)
        && let Ok(raw) = i32::try_from(state.value)
    {
        let output = pedal.normalize(raw);
        ui.add(
            ProgressBar::new(output as f32)
                .desired_width(320.0)
                .text(format!("{:.0}%", output * 100.0)),
        );
    }
    ui.add(Slider::new(&mut pedal.rest_deadzone, 0.0..=0.4).text("Ignore at released end"));
    ui.add(Slider::new(&mut pedal.full_deadzone, 0.0..=0.4).text("Ignore at pressed end"));
    ui.add(Slider::new(&mut pedal.exponent, 0.2..=5.0).text("Response curve"));
    ui.checkbox(&mut pedal.inverted, "Invert");
}

fn steering_reset(config: &mut Config) {
    config.steering.deadzone = 0.0;
    config.steering.exponent = 1.0;
    config.steering.sensitivity = 1.0;
    config.steering.inverted = false;
    for pedal in [&mut config.throttle, &mut config.brake] {
        pedal.rest_deadzone = 0.0;
        pedal.full_deadzone = 0.0;
        pedal.exponent = 1.0;
        pedal.inverted = false;
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let devices = self.monitor.devices();
        let device = devices
            .iter()
            .find(|device| device.is_observed_target)
            .or_else(|| devices.first());

        egui::Panel::left("navigation")
            .resizable(false)
            .exact_size(170.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                for (screen, name) in [
                    (Screen::Device, "Device"),
                    (Screen::Calibrate, "Calibrate"),
                    (Screen::Response, "Steering and pedals"),
                    (Screen::Assign, "Assignments"),
                    (Screen::Later, "Not available yet"),
                ] {
                    ui.selectable_value(&mut self.screen, screen, name);
                }
            });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let can_save =
                    self.config.is_some() && self.unsaved && self.settings_path.is_some();
                if ui
                    .add_enabled(can_save, egui::Button::new("Save"))
                    .clicked()
                {
                    self.save();
                }
                if self.unsaved {
                    ui.colored_label(WARNING, "Unsaved changes");
                } else if let Some((message, color)) = &self.status {
                    ui.colored_label(*color, message);
                }
            });
            if let Some(error) = self.monitor.error() {
                ui.colored_label(ERROR, format!("Can't read devices: {error}"));
            }
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ui, |ui| match self.screen {
            Screen::Device => self.device_panel(ui, device),
            Screen::Calibrate => self.calibrate_panel(ui, device),
            Screen::Response => self.response_panel(ui, device),
            Screen::Assign => self.assign_panel(ui, device),
            Screen::Later => Self::later_panel(ui),
        });
    }
}
