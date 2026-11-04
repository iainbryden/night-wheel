//! Device-independent and game-independent logic.
//!
//! This crate has no dependencies and no `unsafe` code, so it can be tested
//! without a wheel, the game or Windows-specific interfaces.

#![forbid(unsafe_code)]

pub mod assignment;
pub mod calibration;
pub mod config;
pub mod hid_value;
pub mod input;
pub mod listen;
pub mod wizard;
