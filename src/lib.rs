use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait};
use midir::MidiInput;
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;
use std::sync::mpsc;

mod app;
mod audio;
mod input;
mod model;
mod notation;
mod screens;
mod settings;

#[cfg(test)]
mod tests;

use app::*;
use audio::*;
use input::*;
use model::*;
#[cfg(test)]
use notation::*;
use screens::*;
use settings::*;

/// Build and run the Open Band application.
pub fn run() {
    app::run();
}
