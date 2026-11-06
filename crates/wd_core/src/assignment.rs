//! Which control triggers which action.
//!
//! These are saved choices. The game plugin decides which of them reach the game: shifting and
//! changing profile have no game route yet.

use crate::input::{Control, Sample};
use std::collections::{BTreeMap, BTreeSet};

/// Something a control can be assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    ShiftUp,
    ShiftDown,
    TransmissionToggle,
    Boost,
    Handbrake,
    Horn,
    Lights,
    VehicleExit,
    CameraChange,
    RearView,
    RadioNext,
    PauseMenu,
    ProfileNext,
    Autodrive,
    RadioMenu,
    CombatMode,
    RadioPrevious,
    RadioToggle,
    RadioVolumeUp,
    RadioVolumeDown,
    CameraReset,
    ZoomIn,
    ZoomOut,
    Scanner,
    FlyUp,
    FlyDown,
    HudToggle,
}

impl Action {
    /// Every action, in display order.
    pub const ALL: [Self; 27] = [
        Self::ShiftUp,
        Self::ShiftDown,
        Self::TransmissionToggle,
        Self::Boost,
        Self::Handbrake,
        Self::Horn,
        Self::Lights,
        Self::VehicleExit,
        Self::CameraChange,
        Self::RearView,
        Self::RadioNext,
        Self::PauseMenu,
        Self::ProfileNext,
        Self::Autodrive,
        Self::RadioMenu,
        Self::CombatMode,
        Self::RadioPrevious,
        Self::RadioToggle,
        Self::RadioVolumeUp,
        Self::RadioVolumeDown,
        Self::CameraReset,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::Scanner,
        Self::FlyUp,
        Self::FlyDown,
        Self::HudToggle,
    ];

    /// Stable name used in saved settings. Do not change once released.
    pub fn key(self) -> &'static str {
        match self {
            Self::ShiftUp => "shift_up",
            Self::ShiftDown => "shift_down",
            Self::TransmissionToggle => "transmission_toggle",
            Self::Boost => "boost",
            Self::Handbrake => "handbrake",
            Self::Horn => "horn",
            Self::Lights => "lights",
            Self::VehicleExit => "vehicle_exit",
            Self::CameraChange => "camera_change",
            Self::RearView => "rear_view",
            Self::RadioNext => "radio_next",
            Self::PauseMenu => "pause_menu",
            Self::ProfileNext => "profile_next",
            Self::Autodrive => "autodrive",
            Self::RadioMenu => "radio_menu",
            Self::CombatMode => "combat_mode",
            Self::RadioPrevious => "radio_previous",
            Self::RadioToggle => "radio_toggle",
            Self::RadioVolumeUp => "radio_volume_up",
            Self::RadioVolumeDown => "radio_volume_down",
            Self::CameraReset => "camera_reset",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::Scanner => "scanner",
            Self::FlyUp => "fly_up",
            Self::FlyDown => "fly_down",
            Self::HudToggle => "hud_toggle",
        }
    }

    /// Name for screens.
    pub fn label(self) -> &'static str {
        match self {
            Self::ShiftUp => "Shift up",
            Self::ShiftDown => "Shift down",
            Self::TransmissionToggle => "Auto/Manual toggle",
            Self::Boost => "Boost",
            Self::Handbrake => "Handbrake",
            Self::Horn => "Horn",
            Self::Lights => "Lights",
            Self::VehicleExit => "Exit vehicle",
            Self::CameraChange => "Change camera",
            Self::RearView => "Look behind",
            Self::RadioNext => "Next radio station",
            Self::PauseMenu => "Pause menu",
            Self::ProfileNext => "Next profile",
            Self::Autodrive => "Autodrive",
            Self::RadioMenu => "Radio menu",
            Self::CombatMode => "Combat mode",
            Self::RadioPrevious => "Previous radio station",
            Self::RadioToggle => "Radio on/off",
            Self::RadioVolumeUp => "Radio volume up",
            Self::RadioVolumeDown => "Radio volume down",
            Self::CameraReset => "Reset camera",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::Scanner => "Scanner",
            Self::FlyUp => "Climb / start flight",
            Self::FlyDown => "Descend / land",
            Self::HudToggle => "HUD on/off",
        }
    }

    /// Finds an action by its saved name.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.key() == key)
    }
}

/// The saved assignments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    map: BTreeMap<Action, Control>,
}

impl Bindings {
    /// The control assigned to an action.
    pub fn get(&self, action: Action) -> Option<Control> {
        self.map.get(&action).copied()
    }

    /// Other actions already using `control`.
    pub fn conflicts(&self, action: Action, control: Control) -> Vec<Action> {
        self.map
            .iter()
            .filter(|(other, bound)| **other != action && **bound == control)
            .map(|(other, _)| *other)
            .collect()
    }

    /// Assigns a control, replacing the action's previous one. The caller decides what
    /// to do about [`Self::conflicts`]; a deliberate overlap is allowed.
    pub fn assign(&mut self, action: Action, control: Control) {
        self.map.insert(action, control);
    }

    /// Removes an action's assignment.
    pub fn clear(&mut self, action: Action) {
        self.map.remove(&action);
    }

    /// All assignments in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (Action, Control)> + '_ {
        self.map.iter().map(|(action, control)| (*action, *control))
    }

    /// Actions whose control is held in `sample`. For the test panel only.
    pub fn active(&self, sample: &Sample) -> BTreeSet<Action> {
        let held = sample.held();
        self.map
            .iter()
            .filter(|(_, control)| held.contains(control))
            .map(|(action, _)| *action)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ButtonId;

    fn button(usage: u16) -> Control {
        Control::Button(ButtonId { page: 9, usage })
    }

    #[test]
    fn every_action_key_is_unique_and_found() {
        let keys: BTreeSet<&str> = Action::ALL.iter().map(|action| action.key()).collect();
        assert_eq!(keys.len(), Action::ALL.len());
        for action in Action::ALL {
            assert_eq!(Action::from_key(action.key()), Some(action));
        }
        assert_eq!(Action::from_key("nope"), None);
    }

    #[test]
    fn assign_replaces_and_clear_removes() {
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, button(1));
        bindings.assign(Action::Horn, button(2));
        assert_eq!(bindings.get(Action::Horn), Some(button(2)));
        bindings.clear(Action::Horn);
        assert_eq!(bindings.get(Action::Horn), None);
    }

    #[test]
    fn conflicts_name_other_actions_only() {
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, button(1));
        bindings.assign(Action::Lights, button(1));
        bindings.assign(Action::Boost, button(2));
        assert_eq!(
            bindings.conflicts(Action::Horn, button(1)),
            vec![Action::Lights]
        );
        assert!(bindings.conflicts(Action::Horn, button(3)).is_empty());
    }

    #[test]
    fn active_lists_held_actions_including_overlaps() {
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, button(1));
        bindings.assign(Action::Lights, button(1));
        bindings.assign(Action::Boost, button(2));
        let sample = Sample {
            buttons: BTreeSet::from([ButtonId { page: 9, usage: 1 }]),
            ..Sample::default()
        };
        assert_eq!(
            bindings.active(&sample),
            BTreeSet::from([Action::Horn, Action::Lights])
        );
    }
}
