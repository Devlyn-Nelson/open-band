//! Bevy screen modules.
//!
//! Each child module owns the systems, resources, and marker components for one
//! user-facing part of the application. The re-exports keep the application
//! registration readable while the implementation remains localized.

pub(crate) mod editor;
pub(crate) mod home;
pub(crate) mod live;
pub(crate) mod navigation;
pub(crate) mod setup;
pub(crate) mod songs;

pub(crate) use editor::*;
pub(crate) use home::*;
pub(crate) use live::*;
pub(crate) use navigation::*;
pub(crate) use setup::*;
pub(crate) use songs::*;
