//! Read-only wheel input through Windows Raw Input.
//!
//! Raw Input only receives reports. This crate never opens a device for writing,
//! so it has no path for motor or LED output.

#![cfg(windows)]

pub mod device;
pub mod monitor;
pub mod pump;
