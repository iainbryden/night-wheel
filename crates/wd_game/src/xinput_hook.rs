//! Answers the game's gamepad polls with wheel values. The value is computed when the game
//! asks, so nothing waits on a timer. Bound wheel buttons press gamepad buttons the same way.
//! Sends nothing to the wheel. Vibration is not touched.

use crate::{drive, log_file, shared};
use red4ext_rs::{SdkEnv, hooks};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Input::XboxController::{
    XINPUT_CAPABILITIES, XINPUT_DEVSUBTYPE_GAMEPAD, XINPUT_DEVTYPE_GAMEPAD,
    XINPUT_GAMEPAD_BUTTON_FLAGS, XINPUT_STATE,
};
use windows::core::{s, w};

const ERROR_SUCCESS: u32 = 0;
/// A brake value at or above this ends boost. Provisional, about a tenth of its travel.
const BOOST_BRAKE_LIMIT: u8 = 26;
/// The only controller slot the wheel uses.
const WHEEL_USER: u32 = 0;

type GetState = unsafe extern "C" fn(u32, *mut XINPUT_STATE) -> u32;
type GetCapabilities = unsafe extern "C" fn(u32, u32, *mut XINPUT_CAPABILITIES) -> u32;

hooks! {
    static GET_STATE_HOOK: fn(user: u32, state: *mut XINPUT_STATE) -> u32;
    static GET_CAPABILITIES_HOOK: fn(user: u32, flags: u32, capabilities: *mut XINPUT_CAPABILITIES) -> u32;
}

static TARGETS: OnceLock<(GetState, GetCapabilities)> = OnceLock::new();
static SEEN_STATE: AtomicBool = AtomicBool::new(false);
static SEEN_CAPABILITIES: AtomicBool = AtomicBool::new(false);
static PACKET: AtomicU32 = AtomicU32::new(0);
static LAST_OUTPUT: AtomicU64 = AtomicU64::new(u64::MAX);

/// The wheel can answer only with usable settings and a connected wheel.
fn available() -> bool {
    shared::PROFILE.get().is_some() && shared::WHEEL_CONNECTED.load(Ordering::Relaxed)
}

/// Documentation says the packet number changes only when the state changes.
fn packet_for(output: drive::Output, buttons: u16, pad_packet: u32) -> u32 {
    let key = u64::from(output.thumb_x as u16)
        | u64::from(output.left_trigger) << 16
        | u64::from(output.right_trigger) << 24
        | u64::from(buttons) << 32
        | u64::from(pad_packet) << 48;
    if LAST_OUTPUT.swap(key, Ordering::Relaxed) == key {
        PACKET.load(Ordering::Relaxed)
    } else {
        PACKET.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }
}

unsafe extern "C" fn get_state_detour(
    user: u32,
    state: *mut XINPUT_STATE,
    original: GetState,
) -> u32 {
    // SAFETY: the arguments are passed through unchanged from the game.
    let result = unsafe { original(user, state) };
    if user != WHEEL_USER || state.is_null() {
        return result;
    }
    if !SEEN_STATE.swap(true, Ordering::Relaxed) {
        log_file::send("game polled XInputGetState".to_owned());
    }
    shared::record_poll();
    if !available() {
        return result;
    }
    // SAFETY: the game passed `state` to be written, and it was checked for null above.
    let state = unsafe { &mut *state };
    let pad_connected = result == ERROR_SUCCESS;
    if !pad_connected {
        *state = XINPUT_STATE::default();
    }
    let pad = drive::Output {
        thumb_x: state.Gamepad.sThumbLX,
        left_trigger: state.Gamepad.bLeftTrigger,
        right_trigger: state.Gamepad.bRightTrigger,
    };
    let mut output = pad;
    let may_apply = shared::wheel_may_apply();
    let buttons = state.Gamepad.wButtons.0 | shared::pad_buttons(may_apply);
    if may_apply
        && !shared::autopilot()
        && let Some(config) = shared::PROFILE.get()
    {
        output = drive::merge(pad, drive::output(config, shared::raw_axes()));
        shared::record_applied(output.thumb_x);
    }
    shared::set_brake_clear(output.left_trigger < BOOST_BRAKE_LIMIT);
    shared::set_pedals(output.right_trigger, output.left_trigger);
    state.Gamepad.sThumbLX = output.thumb_x;
    state.Gamepad.bLeftTrigger = output.left_trigger;
    state.Gamepad.bRightTrigger = output.right_trigger;
    state.Gamepad.wButtons = XINPUT_GAMEPAD_BUTTON_FLAGS(buttons);
    state.dwPacketNumber = packet_for(output, buttons, state.dwPacketNumber);
    ERROR_SUCCESS
}

unsafe extern "C" fn get_capabilities_detour(
    user: u32,
    flags: u32,
    capabilities: *mut XINPUT_CAPABILITIES,
    original: GetCapabilities,
) -> u32 {
    // SAFETY: the arguments are passed through unchanged from the game.
    let result = unsafe { original(user, flags, capabilities) };
    if user != WHEEL_USER || capabilities.is_null() {
        return result;
    }
    if !SEEN_CAPABILITIES.swap(true, Ordering::Relaxed) {
        log_file::send("game called XInputGetCapabilities".to_owned());
    }
    if result == ERROR_SUCCESS || !available() {
        return result;
    }
    // SAFETY: the game passed `capabilities` to be written, and it was checked for null above.
    let capabilities = unsafe { &mut *capabilities };
    *capabilities = XINPUT_CAPABILITIES::default();
    capabilities.Type = XINPUT_DEVTYPE_GAMEPAD;
    capabilities.SubType = XINPUT_DEVSUBTYPE_GAMEPAD;
    capabilities.Gamepad.wButtons = XINPUT_GAMEPAD_BUTTON_FLAGS(u16::MAX);
    capabilities.Gamepad.bLeftTrigger = u8::MAX;
    capabilities.Gamepad.bRightTrigger = u8::MAX;
    capabilities.Gamepad.sThumbLX = -1;
    capabilities.Gamepad.sThumbLY = -1;
    capabilities.Gamepad.sThumbRX = -1;
    capabilities.Gamepad.sThumbRY = -1;
    ERROR_SUCCESS
}

/// Attaches the hooks. Without them the game simply keeps its normal input.
pub fn attach(env: &SdkEnv) {
    // SAFETY: the name is a static string. Loading a Windows library has no other requirement.
    let Ok(module) = (unsafe { LoadLibraryW(w!("xinput9_1_0.dll")) }) else {
        log_file::send("could not load xinput9_1_0.dll".to_owned());
        return;
    };
    // SAFETY: `module` is valid and the names are static strings.
    let (Some(get_state), Some(get_capabilities)) = (unsafe {
        (
            GetProcAddress(module, s!("XInputGetState")),
            GetProcAddress(module, s!("XInputGetCapabilities")),
        )
    }) else {
        log_file::send("could not find the XInput functions".to_owned());
        return;
    };
    // SAFETY: both signatures follow Microsoft's documentation. On x64 Windows, `system` and
    // `C` are the same calling convention.
    let targets: (GetState, GetCapabilities) = unsafe {
        (
            std::mem::transmute::<unsafe extern "system" fn() -> isize, GetState>(get_state),
            std::mem::transmute::<unsafe extern "system" fn() -> isize, GetCapabilities>(
                get_capabilities,
            ),
        )
    };
    let _ = TARGETS.set(targets);
    // SAFETY: the storage comes from `hooks!` and lives for the whole process, the signatures
    // match, and the loader is single-threaded while plugins load.
    let attached = unsafe {
        (
            env.attach_hook(GET_STATE_HOOK, targets.0, get_state_detour),
            env.attach_hook(GET_CAPABILITIES_HOOK, targets.1, get_capabilities_detour),
        )
    };
    log_file::send(format!(
        "xinput hooks attached state={} capabilities={}",
        attached.0, attached.1
    ));
}

/// Removes the hooks, so no call can land in this library after it unloads.
pub fn detach(env: &SdkEnv) {
    if let Some(&(get_state, get_capabilities)) = TARGETS.get() {
        // SAFETY: both targets were attached above, and this runs during game shutdown.
        unsafe {
            env.detach_hook(get_state);
            env.detach_hook(get_capabilities);
        }
    }
}
