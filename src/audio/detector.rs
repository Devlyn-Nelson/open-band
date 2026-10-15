use crate::*;

/// AI_CODE
pub(crate) struct PolyphonicAudioDetector {
    sample_rate: f32,
    max_pitch_hz: f32,
    processed_samples: usize,
    noise_floor: f32,
    samples: Vec<f32>,
    tracks: Vec<PolyphonicTrack>,
}

/// AI_CODE
struct PolyphonicTrack {
    pitch_hz: f32,
    strength: f32,
    started_sample: usize,
    missed_windows: usize,
}

impl PolyphonicAudioDetector {
    /// AI_CODE
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

    /// AI_CODE
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
                    phase: NotePhase::Sustain,
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
                    phase: NotePhase::Start,
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
                    phase: NotePhase::End,
                    duration_secs: (self.processed_samples - track.started_sample) as f32
                        / self.sample_rate,
                });
            }
        }
        events
    }
}

/// AI_CODE
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

#[test]
/// Tracks more than one pitched voice and reports its playing duration.
fn polyphonic_detector_tracks_chord_duration() {
    let sample_rate = 44_100.0;
    let chord = |amplitude: f32| {
        (0..4096).map(move |index| {
            amplitude * (TAU * 110.0 * index as f32 / sample_rate).sin()
                + amplitude * 0.8 * (TAU * 164.81 * index as f32 / sample_rate).sin()
        })
    };
    let mut detector = PolyphonicAudioDetector::new(sample_rate, 1400.0);
    let starts = detector.detect(chord(0.4));
    assert!(
        starts
            .iter()
            .filter(|event| event.phase == NotePhase::Start)
            .count()
            >= 2
    );

    let updates = detector.detect(chord(0.4).take(2048));
    assert!(
        updates
            .iter()
            .any(|event| { event.phase == NotePhase::Sustain && event.duration_secs > 0.0 })
    );

    let mut silence = detector.detect((0..4096).map(|_| 0.0));
    silence.extend(detector.detect(std::iter::empty::<f32>()));
    assert!(
        silence
            .iter()
            .any(|event| { event.phase == NotePhase::End && event.duration_secs > 0.0 })
    );
}

/// AI_CODE
enum AudioDetectorKind {
    Mono(AudioOnsetDetector),
    Poly(PolyphonicAudioDetector),
    Bass(BassDetector),
}

/// AI_CODE
pub(crate) struct AudioDetector(AudioDetectorKind);

impl AudioDetector {
    /// AI_CODE
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

    /// AI_CODE
    pub(crate) fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        match &mut self.0 {
            AudioDetectorKind::Mono(detector) => detector.detect_with_duration(samples),
            AudioDetectorKind::Poly(detector) => detector.detect(samples),
            AudioDetectorKind::Bass(detector) => detector.detect(samples),
        }
    }
}

/// AI_CODE
struct BassDetector {
    open_frequencies: Vec<f32>,
    mono: AudioOnsetDetector,
    strings_detector: BassStringDetector,
}

impl BassDetector {
    /// AI_CODE
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

    /// AI_CODE
    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        let samples = samples.collect::<Vec<_>>();
        let mut events = self.mono.detect_with_duration(samples.iter().copied());
        let mono_lanes = events
            .iter()
            .filter(|event| event.phase == NotePhase::Start)
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

/// AI_CODE
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

/// AI_CODE
impl BassStringDetector {
    /// AI_CODE
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

    /// AI_CODE
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
                phase: NotePhase::Start,
                duration_secs: 0.0,
            });
        }
        events
    }
}

/// AI_CODE
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

/// AI_CODE
fn harmonic_energy(samples: &[f32], sample_rate: f32, fundamental_hz: f32) -> f32 {
    (1..=4)
        .map(|harmonic| {
            target_frequency_energy(samples, sample_rate, fundamental_hz * harmonic as f32)
        })
        .sum()
}

/// AI_CODE
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
    /// AI_CODE
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

    /// AI_CODE
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
                    phase: NotePhase::End,
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
                phase: NotePhase::Start,
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
                phase: NotePhase::Sustain,
                duration_secs: (self.processed_samples - self.active_started_sample) as f32
                    / self.sample_rate,
            }];
        }
        self.active_pitch_hz = None;
        vec![DetectedNote {
            pitch_hz,
            strength: 0.0,
            noise_floor: self.noise_floor,
            phase: NotePhase::End,
            duration_secs: (self.processed_samples - self.active_started_sample) as f32
                / self.sample_rate,
        }]
    }
}

#[test]
/// Accepts a quiet low-B signal typical of an interface input level.
fn quiet_five_string_low_b_onset_is_detected() {
    let sample_rate = 44_100.0;
    let samples = (0..4096)
        .map(|index| 0.01 * (TAU * 30.87 * index as f32 / sample_rate).sin())
        .collect::<Vec<_>>();
    let mut detector = AudioOnsetDetector {
        sample_rate,
        ..Default::default()
    };
    assert!(
        detector.detect(samples.into_iter()).is_some(),
        "quiet five-string low B should pass the input gate"
    );
}

#[test]
/// Accepts a moderate bass pluck without requiring a hard attack transient.
fn moderate_five_string_pluck_is_detected() {
    let sample_rate = 44_100.0;
    let samples = (0..4096)
        .map(|index| 0.02 * (TAU * 55.0 * index as f32 / sample_rate).sin())
        .collect::<Vec<_>>();
    let mut detector = AudioOnsetDetector {
        sample_rate,
        ..Default::default()
    };
    assert!(
        detector.detect(samples.into_iter()).is_some(),
        "moderate bass pluck should pass the input gate"
    );
}

#[test]
/// Does not retrigger a sustained mono bass note on later analysis windows.
fn sustained_bass_note_produces_one_start() {
    let sample_rate = 44_100.0;
    let tone = |count: usize| {
        (0..count).map(move |index| 0.08 * (TAU * 55.0 * index as f32 / sample_rate).sin())
    };
    let mut detector = AudioOnsetDetector {
        sample_rate,
        ..Default::default()
    };
    let first = detector.detect_with_duration(tone(4096));
    let second = detector.detect_with_duration(tone(4096));
    assert_eq!(
        first
            .iter()
            .filter(|event| event.phase == NotePhase::Start)
            .count(),
        1
    );
    assert_eq!(
        second
            .iter()
            .filter(|event| event.phase == NotePhase::Start)
            .count(),
        0
    );
}

#[test]
/// Allows a quiet but clean bass pluck through the onset gate.
fn quiet_bass_pluck_is_detected() {
    let sample_rate = 44_100.0;
    let samples = (0..4096)
        .map(|index| 0.03 * (TAU * 55.0 * index as f32 / sample_rate).sin())
        .collect::<Vec<_>>();
    let mut detector = AudioOnsetDetector {
        sample_rate,
        ..Default::default()
    };

    let result = detector.detect(samples.into_iter());

    assert!(
        result.is_some(),
        "quiet bass pluck should pass the onset gate"
    );
}

fn detect_recording_pitches(path: &std::path::Path) -> Result<Vec<DetectedNote>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("recording should open: {error}"))?;
    let spec = reader.spec();
    let mut detector = AudioDetector::new(
        InstrumentKind::Strings,
        &[30.87, 41.20, 55.00, 73.42, 98.00],
        DetectorProfile::PerString,
        spec.sample_rate as f32,
    );
    let mut pitches = Vec::new();
    for sample in reader.samples::<i32>() {
        let sample = sample.map_err(|error| format!("recording samples should decode: {error}"))?;
        for note in detector.detect(std::iter::once(sample as f32 / 8_388_608.0)) {
            pitches.push(note);
        }
    }
    Ok(pitches)
}

fn report_recording_errors(test_name: &str, errors: Vec<String>) {
    if !errors.is_empty() {
        panic!(
            "{test_name} found {} recording error(s):\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
}

/// Converts pitch into the corresponding beadg string index.
///
/// 0 => B string
/// 1 => E string
/// 2 => A string
/// 3 => D string
/// 4 => G string
fn pitch_to_beadg_string(pitch_hz: f32) -> Option<usize> {
    match pitch_hz {
        hz if (hz - 41.20).abs() < 1.0 => Some(4),
        hz if (hz - 82.41).abs() < 1.0 => Some(3),
        hz if (hz - 110.00).abs() < 1.0 => Some(2),
        hz if (hz - 146.83).abs() < 1.0 => Some(1),
        hz if (hz - 196.00).abs() < 1.0 => Some(0),
        _ => None,
    }
}

#[test]
// #[ignore = "the supplied recordings currently expose pitch-detector false positives; run explicitly while tuning DSP"]
/// Checks stable single-string detection and ordered multi-string detection.
fn supplied_bass_recordings_detect_expected_open_strings() {
    let recording_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("recordings");
    let mut paths = std::fs::read_dir(&recording_directory)
        .expect("recordings directory should exist")
        .map(|entry| entry.expect("recording entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("open-"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut errors = Vec::new();
    for path in paths {
        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(file_name) => file_name,
            None => {
                errors.push(format!(
                    "{}: recording filename should be valid UTF-8",
                    path.display()
                ));
                continue;
            }
        };
        let result = (|| -> Result<(), String> {
            let expected_strings = file_name
                .strip_prefix("open-")
                .and_then(|name| name.strip_suffix(".wav"))
                .and_then(|name| name.split('-').next())
                .ok_or_else(|| {
                    "recording should use the open-<strings>-<suffix>.wav format".to_string()
                })?;
            let expected_lanes = expected_strings
                .chars()
                .map(|string| match string {
                    'b' => Ok(0),
                    'e' => Ok(1),
                    'a' => Ok(2),
                    'd' => Ok(3),
                    'g' => Ok(4),
                    _ => Err(format!("unknown bass string '{string}'")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if expected_lanes.len() == 1 {
                let detected_pitches = detect_recording_pitches(&path)?;
                for (i, note) in detected_pitches.iter().enumerate() {
                    let expected = expected_lanes[0];
                    let Some(detected) = pitch_to_beadg_string(note.pitch_hz) else {
                        return Err(format!(
                            "pluck {i} could not be mapped to a string (pitch {})",
                            note.pitch_hz,
                        ));
                    };
                    if detected != expected {
                        return Err(format!(
                            "pluck {i} is not on the expected string: expected {expected}, got {detected}",
                        ));
                    }
                }
                let durations = detected_pitches
                    .iter()
                    .filter_map(|note| {
                        if note.phase.is_end() {
                            Some(note.duration_secs)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if durations.len() != 12 {
                    return Err(format!(
                        "should contain exactly 12 plucks; detected {} plucks",
                        durations.len()
                    ));
                }
                for (i, duration) in durations.iter().enumerate() {
                    if *duration <= 0.05 {
                        return Err(format!("pluck {i} has duration <= 0.05: {duration}"));
                    } else if *duration > 0.75 {
                        return Err(format!("pluck {i} has duration > 0.75: {duration}"));
                    }
                }
            } else {
                let detected_pitches = detect_recording_pitches(&path)?
                    .into_iter()
                    .filter_map(|note| {
                        if note.phase.is_start() {
                            Some(note.pitch_hz)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                let bass5 = Instrument {
                    slot: 0,
                    kind: InstrumentKind::Strings,
                    strings: 5,
                };
                let bass5_open_frequencies = [30.87, 41.20, 55.00, 73.42, 98.00];
                let mut detected_lanes = detected_pitches
                    .iter()
                    .map(|pitch| pitch_to_lane(&bass5, &bass5_open_frequencies, *pitch))
                    .collect::<Vec<_>>();
                detected_lanes.dedup();
                if detected_lanes != expected_lanes {
                    return Err(format!(
                        "unexpected string order; expected={expected_lanes:?}, detected={detected_lanes:?}, pitches={detected_pitches:?}"
                    ));
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{file_name}: {error}"));
        }
    }
    report_recording_errors(
        "supplied_bass_recordings_detect_expected_open_strings",
        errors,
    );
}

#[test]
#[ignore = "run explicitly as fret recordings are added and the detector is tuned"]
/// Checks fret recordings for open string through fret 24 in order.
fn supplied_bass_fret_recordings_detect_open_through_fret_24() {
    let recording_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("recordings");
    let mut paths = std::fs::read_dir(&recording_directory)
        .expect("recordings directory should exist")
        .map(|entry| entry.expect("recording entry should be readable").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("fret-") && name.ends_with(".wav"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut errors = Vec::new();
    if paths.is_empty() {
        errors.push("expected at least one fret-<string>.wav recording".to_string());
    }

    for path in paths {
        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(file_name) => file_name,
            None => {
                errors.push(format!(
                    "{}: recording filename should be valid UTF-8",
                    path.display()
                ));
                continue;
            }
        };
        let result = (|| -> Result<(), String> {
            let string = file_name
                .strip_prefix("fret-")
                .and_then(|name| name.strip_suffix(".wav"))
                .and_then(|name| name.split('-').next())
                .ok_or_else(|| {
                    "fret recording should use fret-<string>-<suffix>.wav format".to_string()
                })?;
            let open_pitch = match string {
                "b" => 30.87,
                "e" => 41.20,
                "a" => 55.00,
                "d" => 73.42,
                "g" => 98.00,
                _ => return Err(format!("unknown bass string '{string}'")),
            };
            let expected_pitches = (0..=24)
                .map(|fret| open_pitch * 2.0_f32.powf(fret as f32 / 12.0))
                .collect::<Vec<_>>();
            let mut detected_frets = detect_recording_pitches(&path)?
                .into_iter()
                .filter_map(|note| {
                    if note.phase.is_start() {
                        Some(note.pitch_hz)
                    } else {
                        None
                    }
                })
                .map(|pitch| {
                    expected_pitches
                        .iter()
                        .enumerate()
                        .min_by(|(_, left), (_, right)| {
                            (pitch / *left)
                                .ln()
                                .abs()
                                .total_cmp(&(pitch / *right).ln().abs())
                        })
                        .map(|(fret, _)| fret)
                        .ok_or_else(|| "expected fret list should not be empty".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            detected_frets.dedup();
            let expected_frets = (0..=24).collect::<Vec<_>>();
            if detected_frets != expected_frets {
                return Err(format!(
                    "unexpected fret sequence; expected={expected_frets:?}, detected={detected_frets:?}"
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{file_name}: {error}"));
        }
    }
    report_recording_errors(
        "supplied_bass_fret_recordings_detect_open_through_fret_24",
        errors,
    );
}
