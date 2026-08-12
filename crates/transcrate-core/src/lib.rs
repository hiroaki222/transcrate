//! Core logic shared by the Transcrate CLI and GUI.
//!
//! Everything that decides *what* to do lives here; the command line and the
//! Tauri backend are thin shells over this crate. One implementation is what
//! keeps the two front-ends from drifting apart.

pub mod compat;
pub mod convert;
pub mod device;
pub mod files;
pub mod parallel;
pub mod plan;
pub mod probe;
pub mod usb;

pub use compat::{AudioSpec, Issue, check};
pub use device::{
    Codec, DEVICES, DeviceProfile, FileSystem, FormatSupport, LossyLimits, Support, by_id,
};
