use crate::*;

pub(crate) struct PolyphonicAudioDetector {
    sample_rate: f32,
    max_pitch_hz: f32,
    processed_samples: usize,
    noise_floor: f32,
    samples: Vec<f32>,
    tracks: Vec<PolyphonicTrack>,
}

struct PolyphonicTrack {
    pitch_hz: f32,
    strength: f32,
    started_sample: usize,
    missed_windows: usize,
}

impl PolyphonicAudioDetector {
    pub(crate) fn new(sample_rate: f32, max_pitch_hz: f32) -> Self {
        Self {
            sample_rate,
            max_pitch_hz,
            processed_samples: 0,
            noise_floor: 0.0,
            samples: Vec::new(),
            tracks: Vec::new(),
        }
    }

    pub(crate) fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return Vec::new();
        }
        let window = self.samples[..4096].to_vec();
        self.samples.drain(..2048);
        self.processed_samples += 2048;

        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        self.noise_floor = self.noise_floor * 0.995 + level * 0.005;
        let mut peaks = spectral_peaks(
            &window,
            self.sample_rate,
            (self.noise_floor * 2.5).max(0.008),
        );
        peaks.retain(|(pitch, _)| *pitch <= self.max_pitch_hz);
        let mut events = Vec::new();
        let mut matched = vec![false; self.tracks.len()];

        for (pitch_hz, strength) in peaks {
            let matching_track = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(index, track)| {
                    !matched[*index] && (pitch_hz / track.pitch_hz).log2().abs() <= 90.0 / 1200.0
                })
                .min_by(|(_, left), (_, right)| {
                    (pitch_hz / left.pitch_hz)
                        .log2()
                        .abs()
                        .total_cmp(&(pitch_hz / right.pitch_hz).log2().abs())
                })
                .map(|(index, _)| index);

            if let Some(index) = matching_track {
                matched[index] = true;
                let track = &mut self.tracks[index];
                track.pitch_hz = pitch_hz;
                track.strength = strength;
                track.missed_windows = 0;
                events.push(DetectedNote {
                    pitch_hz,
                    strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Updated,
                    duration_secs: (self.processed_samples - track.started_sample) as f32
                        / self.sample_rate,
                });
            } else if self.tracks.len() < 6 {
                self.tracks.push(PolyphonicTrack {
                    pitch_hz,
                    strength,
                    started_sample: self.processed_samples,
                    missed_windows: 0,
                });
                matched.push(true);
                events.push(DetectedNote {
                    pitch_hz,
                    strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Started,
                    duration_secs: 0.0,
                });
            }
        }

        for index in (0..self.tracks.len()).rev() {
            if matched.get(index).copied().unwrap_or(false) {
                continue;
            }
            self.tracks[index].missed_windows += 1;
            if self.tracks[index].missed_windows >= 2 {
                let track = self.tracks.remove(index);
                events.push(DetectedNote {
                    pitch_hz: track.pitch_hz,
                    strength: track.strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Ended,
                    duration_secs: (self.processed_samples - track.started_sample) as f32
                        / self.sample_rate,
                });
            }
        }
        events
    }
}

fn spectral_peaks(samples: &[f32], sample_rate: f32, threshold: f32) -> Vec<(f32, f32)> {
    let fft_size = samples.len().next_power_of_two() * 4;
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut spectrum = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let window = 0.5
                * (1.0 - (std::f32::consts::TAU * index as f32 / (samples.len() - 1) as f32).cos());
            Complex::new(sample * window, 0.0)
        })
        .chain(std::iter::repeat(Complex::new(0.0, 0.0)).take(fft_size - samples.len()))
        .collect::<Vec<_>>();
    fft.process(&mut spectrum);
    let minimum_bin = (30.0 * fft_size as f32 / sample_rate).ceil() as usize;
    let maximum_bin = (1400.0 * fft_size as f32 / sample_rate)
        .floor()
        .min((fft_size / 2 - 1) as f32) as usize;
    let magnitudes = (minimum_bin..=maximum_bin)
        .map(|bin| spectrum[bin].norm() / samples.len() as f32 * 2.0)
        .collect::<Vec<_>>();
    let mut peaks = (1..magnitudes.len().saturating_sub(1))
        .filter_map(|offset| {
            let magnitude = magnitudes[offset];
            (magnitude > threshold
                && magnitude >= magnitudes[offset - 1]
                && magnitude >= magnitudes[offset + 1])
                .then_some((minimum_bin + offset, magnitude))
        })
        .collect::<Vec<_>>();
    peaks.sort_by(|left, right| right.1.total_cmp(&left.1));
    peaks
        .into_iter()
        .take(6)
        .map(|(bin, strength)| {
            (
                bin as f32 * sample_rate / fft_size as f32,
                (strength * 8.0).clamp(0.15, 1.0),
            )
        })
        .collect()
}

enum AudioDetectorKind {
    Mono(AudioOnsetDetector),
    Poly(PolyphonicAudioDetector),
    Bass(BassDetector),
}

pub(crate) struct AudioDetector(AudioDetectorKind);

impl AudioDetector {
    pub(crate) fn new(
        kind: InstrumentKind,
        open_frequencies: &[f32],
        profile: DetectorProfile,
        sample_rate: f32,
    ) -> Self {
        match kind {
            InstrumentKind::Voice | InstrumentKind::Percussion => {
                Self(AudioDetectorKind::Mono(AudioOnsetDetector {
                    sample_rate,
                    min_pitch_hz: 60.0,
                    max_pitch_hz: 1400.0,
                    ..Default::default()
                }))
            }
            InstrumentKind::Strings => match profile {
                DetectorProfile::Polyphonic => Self(AudioDetectorKind::Poly(
                    PolyphonicAudioDetector::new(sample_rate, 1400.0),
                )),
                DetectorProfile::PerString => Self(AudioDetectorKind::Bass(BassDetector::new(
                    open_frequencies.to_vec(),
                    sample_rate,
                ))),
            },
        }
    }

    pub(crate) fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        match &mut self.0 {
            AudioDetectorKind::Mono(detector) => detector.detect_with_duration(samples),
            AudioDetectorKind::Poly(detector) => detector.detect(samples),
            AudioDetectorKind::Bass(detector) => detector.detect(samples),
        }
    }
}

struct BassDetector {
    open_frequencies: Vec<f32>,
    mono: AudioOnsetDetector,
    strings_detector: BassStringDetector,
}

impl BassDetector {
    fn new(open_frequencies: Vec<f32>, sample_rate: f32) -> Self {
        Self {
            mono: AudioOnsetDetector {
                sample_rate,
                min_pitch_hz: 30.0,
                max_pitch_hz: 500.0,
                ..Default::default()
            },
            strings_detector: BassStringDetector::new(sample_rate, open_frequencies.clone()),
            open_frequencies,
        }
    }

    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        let samples = samples.collect::<Vec<_>>();
        let mut events = self.mono.detect_with_duration(samples.iter().copied());
        let mono_lanes = events
            .iter()
            .filter(|event| event.phase == NotePhase::Started)
            .map(|event| string_lane(&self.open_frequencies, event.pitch_hz))
            .collect::<Vec<_>>();
        for note in self.strings_detector.detect(samples.into_iter()) {
            if !mono_lanes.contains(&string_lane(&self.open_frequencies, note.pitch_hz)) {
                events.push(note);
            }
        }
        events
    }
}

struct BassStringDetector {
    sample_rate: f32,
    targets: Vec<f32>,
    samples: Vec<f32>,
    processed_samples: usize,
    baseline: Vec<f32>,
    active_lane: Option<usize>,
    pending_lane: Option<usize>,
    pending_count: usize,
    last_event_sample: Option<usize>,
}

impl BassStringDetector {
    fn new(sample_rate: f32, targets: Vec<f32>) -> Self {
        let count = targets.len();
        Self {
            sample_rate,
            targets,
            samples: Vec::new(),
            processed_samples: 0,
            baseline: vec![0.0; count],
            active_lane: None,
            pending_lane: None,
            pending_count: 0,
            last_event_sample: None,
        }
    }

    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return Vec::new();
        }
        let window = self.samples[..4096].to_vec();
        self.samples.drain(..2048);
        self.processed_samples += 2048;

        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        let energies = self
            .targets
            .iter()
            .map(|frequency| {
                harmonic_energy(&window, self.sample_rate, *frequency) / window.len() as f32
            })
            .collect::<Vec<_>>();
        for (baseline, energy) in self.baseline.iter_mut().zip(&energies) {
            if *energy < *baseline {
                *baseline = *baseline * 0.9 + *energy * 0.1;
            } else {
                *baseline = *baseline * 0.999 + *energy * 0.001;
            }
        }

        if level < 0.005 {
            self.active_lane = None;
            self.pending_lane = None;
            self.pending_count = 0;
            return Vec::new();
        }

        let winner = energies
            .iter()
            .zip(&self.baseline)
            .enumerate()
            .filter(|(_, (energy, baseline))| {
                **energy / baseline.max(1e-6) > 4.5 && **energy > 0.012
            })
            .max_by(|(_, (left, _)), (_, (right, _))| left.total_cmp(right))
            .map(|(lane, _)| lane);

        let mut events = Vec::new();
        let Some(lane) = winner else {
            if level < 0.01 {
                self.active_lane = None;
            }
            self.pending_lane = None;
            self.pending_count = 0;
            return events;
        };
        if self.active_lane == Some(lane) {
            self.pending_lane = None;
            self.pending_count = 0;
            return events;
        }
        if self.pending_lane == Some(lane) {
            self.pending_count += 1;
        } else {
            self.pending_lane = Some(lane);
            self.pending_count = 1;
        }
        let ready = self.last_event_sample.map_or(true, |event| {
            self.processed_samples.saturating_sub(event) > (self.sample_rate * 0.2) as usize
        });
        if ready && self.pending_count >= 2 {
            self.active_lane = Some(lane);
            self.last_event_sample = Some(self.processed_samples);
            events.push(DetectedNote {
                pitch_hz: self.targets[lane],
                strength: (level * 4.0).clamp(0.15, 1.0),
                noise_floor: 0.0,
                phase: NotePhase::Started,
                duration_secs: 0.0,
            });
        }
        events
    }
}

fn target_frequency_energy(samples: &[f32], sample_rate: f32, frequency: f32) -> f32 {
    let last = (samples.len() - 1).max(1) as f32;
    let (real, imaginary) =
        samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(real, imaginary), (index, sample)| {
                let taper = 0.5 * (1.0 - (std::f32::consts::TAU * index as f32 / last).cos());
                let sample = sample * taper;
                let phase = std::f32::consts::TAU * frequency * index as f32 / sample_rate;
                (
                    real + sample * phase.cos(),
                    imaginary + sample * phase.sin(),
                )
            });
    real.hypot(imaginary)
}

fn harmonic_energy(samples: &[f32], sample_rate: f32, fundamental_hz: f32) -> f32 {
    (1..=4)
        .map(|harmonic| {
            target_frequency_energy(samples, sample_rate, fundamental_hz * harmonic as f32)
        })
        .sum()
}

#[derive(Default)]
pub(crate) struct AudioOnsetDetector {
    pub(crate) average: f32,
    pub(crate) noise_floor: f32,
    pub(crate) processed_samples: usize,
    pub(crate) last_event_sample: Option<usize>,
    pub(crate) last_pitch_hz: Option<f32>,
    pub(crate) pending_pitch_hz: Option<f32>,
    pub(crate) pending_pitch_count: usize,
    pub(crate) sample_rate: f32,
    pub(crate) samples: Vec<f32>,
    pub(crate) last_level: f32,
    pub(crate) active_pitch_hz: Option<f32>,
    pub(crate) active_started_sample: usize,
    pub(crate) silent_windows: usize,
    pub(crate) min_pitch_hz: f32,
    pub(crate) max_pitch_hz: f32,
}

impl AudioOnsetDetector {
    pub(crate) fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Option<(f32, f32, f32)> {
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return None;
        }

        let window = self.samples[..4096].to_vec();
        self.samples.drain(..2048);
        self.processed_samples += 2048;
        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        self.last_level = level;
        if level < self.noise_floor {
            self.noise_floor = self.noise_floor * 0.9 + level * 0.1;
        } else {
            self.noise_floor = self.noise_floor * 0.999 + level * 0.001;
        }
        self.average = self.average * 0.96 + level * 0.04;
        let (pitch_hz, confidence) = estimate_pitch_with_confidence(
            &window,
            self.sample_rate,
            if self.min_pitch_hz > 0.0 {
                self.min_pitch_hz
            } else {
                30.0
            },
            if self.max_pitch_hz > 0.0 {
                self.max_pitch_hz
            } else {
                1400.0
            },
        )
        .map_or((None, 0.0), |(pitch, confidence)| (Some(pitch), confidence));
        let pitch_hz = if confidence >= CONFIDENT_PITCH_THRESHOLD {
            pitch_hz
        } else {
            None
        };
        let ready = self.last_event_sample.map_or(true, |event| {
            self.processed_samples.saturating_sub(event) > (self.sample_rate * 0.12) as usize
        });
        let minimum_level = (self.noise_floor * 2.5).max(0.005);
        let pitch_changed = self
            .last_pitch_hz
            .zip(pitch_hz)
            .is_some_and(|(last, current)| (current / last).log2().abs() > 150.0 / 1200.0);
        let confirmed_pitch_change = if pitch_changed {
            let Some(current_pitch) = pitch_hz else {
                return None;
            };
            if self
                .pending_pitch_hz
                .is_some_and(|pending| (current_pitch / pending).log2().abs() <= 80.0 / 1200.0)
            {
                self.pending_pitch_count += 1;
            } else {
                self.pending_pitch_hz = Some(current_pitch);
                self.pending_pitch_count = 1;
            }
            self.pending_pitch_count >= 3
        } else {
            self.pending_pitch_hz = None;
            self.pending_pitch_count = 0;
            false
        };
        let initial_attack = self.active_pitch_hz.is_none() && level > self.average * 1.15;
        if level > minimum_level && ready && (initial_attack || confirmed_pitch_change) {
            self.last_event_sample = Some(self.processed_samples);
            if let Some(pitch_hz) = pitch_hz {
                self.last_pitch_hz = Some(pitch_hz);
                self.pending_pitch_hz = None;
                self.pending_pitch_count = 0;
                Some(((level * 4.0).clamp(0.15, 1.0), pitch_hz, self.noise_floor))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(crate) fn detect_with_duration(
        &mut self,
        samples: impl Iterator<Item = f32>,
    ) -> Vec<DetectedNote> {
        let onset = self.detect(samples);
        if let Some((strength, pitch_hz, noise_floor)) = onset {
            let mut events = Vec::new();
            if let Some(active_pitch_hz) = self.active_pitch_hz {
                events.push(DetectedNote {
                    pitch_hz: active_pitch_hz,
                    strength: self.last_level,
                    noise_floor,
                    phase: NotePhase::Ended,
                    duration_secs: (self.processed_samples - self.active_started_sample) as f32
                        / self.sample_rate,
                });
            }
            self.active_started_sample = self.processed_samples;
            self.active_pitch_hz = Some(pitch_hz);
            self.silent_windows = 0;
            events.push(DetectedNote {
                pitch_hz,
                strength,
                noise_floor,
                phase: NotePhase::Started,
                duration_secs: (self.processed_samples - self.active_started_sample) as f32
                    / self.sample_rate,
            });
            return events;
        }

        let Some(pitch_hz) = self.active_pitch_hz else {
            return Vec::new();
        };
        let minimum_level = (self.noise_floor * 2.5).max(0.005);
        if self.last_level <= minimum_level {
            self.silent_windows += 1;
        } else {
            self.silent_windows = 0;
        }
        if self.silent_windows < 2 {
            return vec![DetectedNote {
                pitch_hz,
                strength: (self.last_level * 4.0).clamp(0.15, 1.0),
                noise_floor: self.noise_floor,
                phase: NotePhase::Updated,
                duration_secs: (self.processed_samples - self.active_started_sample) as f32
                    / self.sample_rate,
            }];
        }
        self.active_pitch_hz = None;
        vec![DetectedNote {
            pitch_hz,
            strength: 0.0,
            noise_floor: self.noise_floor,
            phase: NotePhase::Ended,
            duration_secs: (self.processed_samples - self.active_started_sample) as f32
                / self.sample_rate,
        }]
    }
}
