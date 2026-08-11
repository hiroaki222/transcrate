//! Core logic shared by the Transcrate CLI and GUI.
//!
//! Everything that decides *what* to do lives here; the command line and the
//! Tauri backend are thin shells over this crate. One implementation is what
//! keeps the two front-ends from drifting apart.

pub mod compat;
pub mod device;

pub use compat::{Issue, OutputSpec, check};
pub use device::{
    Codec, DEVICES, DeviceProfile, FileSystem, FormatSupport, LossyLimits, Support, by_id,
};
