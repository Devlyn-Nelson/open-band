use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait};
use midir::MidiInput;
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;
use std::sync::mpsc;

mod app;
mod audio;
mod chart;
mod detector;
mod domain;
mod gameplay;
mod input;
mod menu;
mod navigation;
mod runtime;
mod settings;
mod setup;
mod state;

#[cfg(test)]
mod tests;

use audio::*;
use chart::*;
use detector::*;
use domain::*;
use gameplay::*;
use input::*;
use menu::*;
use navigation::*;
use runtime::*;
use settings::*;
use setup::*;
use state::*;

fn main() {
    app::run();
}
