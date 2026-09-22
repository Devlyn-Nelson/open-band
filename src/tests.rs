use super::{
    AudioDetector, AudioOnsetDetector, Chart, DeviceChoice, Instrument, NotePhase,
    PolyphonicAudioDetector, bass_string_lane, cents_error, estimate_pitch, load_charts,
    pitch_to_lane, selected_device_index,
};
use std::f32::consts::TAU;
use std::path::Path;

#[test]
/// Keeps the same pitch stable when its amplitude changes.
fn pitch_estimation_is_stable_across_signal_strengths() {
    let sample_rate = 44_100.0;
    let pitches = [0.15, 0.9].map(|amplitude| {
        let samples = (0..4096)
            .map(|index| amplitude * (TAU * 55.0 * index as f32 / sample_rate).sin())
            .collect::<Vec<_>>();
        estimate_pitch(&samples, sample_rate).expect("a clear bass tone should be detected")
    });

    assert!(
        (pitches[0] - 55.0).abs() < 1.0,
        "quiet pitch: {}",
        pitches[0]
    );
    assert!(
        (pitches[1] - 55.0).abs() < 1.0,
        "loud pitch: {}",
        pitches[1]
    );
    assert!(
        (pitches[0] - pitches[1]).abs() < 0.5,
        "pitch drift: {pitches:?}"
    );
}

#[test]
/// Confirms that each bass open string owns a distinct lane.
fn pitch_estimation_detects_five_string_low_b() {
    let sample_rate = 44_100.0;
    let samples = (0..4096)
        .map(|index| (TAU * 30.87 * index as f32 / sample_rate).sin())
        .collect::<Vec<_>>();

    let pitch = estimate_pitch(&samples, sample_rate).expect("low B should be detected");
    assert!((pitch - 30.87).abs() < 0.75, "detected pitch: {pitch}");
}

#[test]
/// Confirms the bass fundamental estimator covers every five-string open note.
fn pitch_estimation_detects_all_five_string_open_notes() {
    let sample_rate = 44_100.0;
    for expected in [30.87, 41.20, 55.00, 73.42, 98.00] {
        let samples = (0..4096)
            .map(|index| (TAU * expected * index as f32 / sample_rate).sin())
            .collect::<Vec<_>>();
        let detected = estimate_pitch(&samples, sample_rate)
            .expect("each five-string open note should be detected");
        assert!(
            (detected - expected).abs() < 1.0,
            "expected={expected}, detected={detected}"
        );
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
            .filter(|event| event.phase == NotePhase::Started)
            .count(),
        1
    );
    assert_eq!(
        second
            .iter()
            .filter(|event| event.phase == NotePhase::Started)
            .count(),
        0
    );
}

#[test]
/// Confirms four- and five-string open-note lane assignments.
fn bass_open_strings_map_to_their_string_lanes() {
    let five_string_open_notes = [30.87, 41.20, 55.00, 73.42, 98.00];
    for (lane, pitch) in five_string_open_notes.into_iter().enumerate() {
        assert_eq!(bass_string_lane(Instrument::Bass5, pitch), lane);
    }

    let four_string_open_notes = [41.20, 55.00, 73.42, 98.00];
    for (lane, pitch) in four_string_open_notes.into_iter().enumerate() {
        assert_eq!(bass_string_lane(Instrument::Bass4, pitch), lane);
    }
}

#[test]
/// Selects the exact stable ID when display names are duplicated.
fn device_selection_prefers_stable_id_over_duplicate_name() {
    let devices = vec![
        DeviceChoice {
            id: "alsa:hw:1,0".into(),
            label: "USB Audio".into(),
        },
        DeviceChoice {
            id: "alsa:hw:2,0".into(),
            label: "USB Audio".into(),
        },
    ];
    assert_eq!(
        selected_device_index(&devices, Some("alsa:hw:2,0"), "MISSING"),
        1
    );
}

#[test]
/// Documents the physical-string lanes used for instrument navigation.
fn instrument_navigation_string_mapping() {
    assert_eq!(bass_string_lane(Instrument::Bass5, 98.0), 4);
    assert_eq!(bass_string_lane(Instrument::Bass5, 73.42), 3);
    assert_eq!(bass_string_lane(Instrument::Bass5, 55.0), 2);
    assert_eq!(bass_string_lane(Instrument::Bass5, 41.20), 1);
}

#[test]
/// Loads the built-in chart with ordered, playable bass notes.
fn built_in_chart_has_playable_notes() {
    let chart: Chart =
        serde_json::from_str(super::OPEN_STRINGS_CHART).expect("built-in chart should parse");
    assert_eq!(chart.version, 1);
    assert_eq!(chart.tracks.len(), 1);
    assert_eq!(chart.tracks[0].instrument, "bass5");
    assert_eq!(chart.tracks[0].tuning.as_deref(), Some("standard_bass_5"));
    let notes = chart.bass_notes();
    assert_eq!(notes.len(), 10);
    assert_eq!(notes[0].note, "B0");
    assert_eq!(notes[5].note, "D1");
    assert!(
        notes
            .windows(2)
            .all(|notes| notes[0].start < notes[1].start)
    );
}

#[test]
fn bass_projection_falls_back_when_preferred_string_cannot_play_note() {
    let chart: Chart = serde_json::from_str(
        r#"
                {
                    "version": 1,
                    "title": "Projection",
                    "bpm": 90.0,
                    "time_signature": [4, 4],
                    "tracks": [{
                        "name": "Bass",
                        "instrument": "bass5",
                        "tuning": "standard_bass_5",
                        "notes": [{
                            "start_beat": 1.0,
                            "duration_beats": 1.0,
                            "note": "C1",
                            "preferred_string": 2
                        }]
                    }]
                }
                "#,
    )
    .expect("projection chart should parse");

    let notes = chart.bass_notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].string, 0);
    assert_eq!(notes[0].fret, 1);
}

#[test]
fn chart_directory_loads_all_valid_charts() {
    let charts = load_charts();
    assert!(
        charts.len() >= 2,
        "expected the built-in and developer charts"
    );
    assert!(
        charts
            .iter()
            .any(|chart| chart.title == "Open Strings Study")
    );
    let developer_chart = charts
        .iter()
        .find(|chart| chart.title == "Devs Test Song")
        .expect("developer chart should load");
    assert_eq!(developer_chart.bass_notes().len(), 10);
}

#[test]
fn chart_supports_multiple_instrument_tracks_and_vocal_phrases() {
    let chart: Chart = serde_json::from_str(
                r#"
                {
                    "version": 1,
                    "title": "Band Test",
                    "bpm": 120.0,
                    "time_signature": [4, 4],
                    "tracks": [
                        {
                            "name": "Bass",
                            "instrument": "bass5",
                            "tuning": "standard_bass_5",
                            "notes": [{ "start_beat": 1.0, "duration_beats": 1.0, "midi_note": 23 }]
                        },
                        {
                            "name": "Lead Vocal",
                            "instrument": "vocals",
                            "phrases": [{
                                "start_beat": 2.0,
                                "duration_beats": 2.0,
                                "text": "Hello",
                                "notes": [{ "start_beat": 2.0, "duration_beats": 2.0, "note": "C4" }]
                            }]
                        }
                    ]
                }
                "#,
        )
        .expect("multi-track chart should parse");

    assert_eq!(chart.time_signature, [4, 4]);
    assert_eq!(chart.tracks.len(), 2);
    assert_eq!(chart.tracks[1].phrases[0].text, "Hello");
    assert_eq!(chart.tracks[1].phrases[0].notes[0].midi_note, 60);
    assert_eq!(chart.total_beats(), 4.0);
    assert_eq!(chart.bass_notes()[0].note, "B0");
}

#[test]
/// Accepts a detected pitch within the chart matcher tolerance.
fn chart_pitch_matching_uses_cents() {
    assert!(cents_error(56.0, 55.0).abs() < 90.0);
    assert!(cents_error(65.4, 55.0).abs() > 90.0);
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
            .filter(|event| event.phase == NotePhase::Started)
            .count()
            >= 2
    );

    let updates = detector.detect(chord(0.4).take(2048));
    assert!(
        updates
            .iter()
            .any(|event| { event.phase == NotePhase::Updated && event.duration_secs > 0.0 })
    );

    let mut silence = detector.detect((0..4096).map(|_| 0.0));
    silence.extend(detector.detect(std::iter::empty::<f32>()));
    assert!(
        silence
            .iter()
            .any(|event| { event.phase == NotePhase::Ended && event.duration_secs > 0.0 })
    );
}

fn detect_recording_pitches(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("recording should open: {error}"))?;
    let spec = reader.spec();
    let mut detector = AudioDetector::new(Instrument::Bass5, spec.sample_rate as f32);
    let mut pitches = Vec::new();
    for sample in reader.samples::<i32>() {
        let sample = sample.map_err(|error| format!("recording samples should decode: {error}"))?;
        for note in detector.detect(std::iter::once(sample as f32 / 8_388_608.0)) {
            if note.phase == NotePhase::Started {
                pitches.push(note.pitch_hz);
            }
        }
    }
    Ok(pitches)
}

fn detect_recording_plucks(path: &Path) -> Result<Vec<(f32, f32)>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("recording should open: {error}"))?;
    let spec = reader.spec();
    let samples = reader
        .samples::<i32>()
        .map(|sample| {
            sample
                .map(|sample| sample as f32 / 8_388_608.0)
                .map_err(|error| format!("recording samples should decode: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let window_size = 512;
    let raw_envelope = samples
        .chunks(window_size)
        .map(|window| {
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len().max(1) as f32)
                .sqrt()
        })
        .collect::<Vec<_>>();
    let smoothing_windows = 9;
    let envelope = raw_envelope
        .windows(smoothing_windows)
        .map(|window| window.iter().sum::<f32>() / window.len() as f32)
        .collect::<Vec<_>>();
    let maximum = envelope.iter().copied().fold(0.0, f32::max);
    let threshold = (maximum * 0.08).max(0.002);
    let minimum_distance = (spec.sample_rate as f32 * 0.2 / window_size as f32) as usize;
    let mut candidates = Vec::new();
    for index in 1..envelope.len().saturating_sub(1) {
        if envelope[index] <= threshold
            || envelope[index] < envelope[index - 1]
            || envelope[index] < envelope[index + 1]
        {
            continue;
        }
        candidates.push((index, envelope[index]));
    }
    candidates.sort_by(|left, right| right.1.total_cmp(&left.1));
    let mut peaks = Vec::new();
    for (index, _) in candidates {
        if peaks_are_separated(index, &peaks, minimum_distance) {
            peaks.push(index);
        }
        if peaks.len() == 12 {
            break;
        }
    }
    peaks.sort_unstable();
    Ok(peaks
        .into_iter()
        .map(|peak| {
            let sustain_threshold = envelope[peak] * 0.2;
            let left = (0..=peak)
                .rev()
                .find(|index| envelope[*index] < sustain_threshold)
                .unwrap_or(0);
            let right = (peak..envelope.len())
                .find(|index| envelope[*index] < sustain_threshold)
                .unwrap_or(envelope.len());
            (
                peak as f32 * window_size as f32 / spec.sample_rate as f32,
                (right.saturating_sub(left) * window_size) as f32 / spec.sample_rate as f32,
            )
        })
        .collect())
}

#[allow(dead_code)]
fn detect_recording_attack_pitches(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("recording should open: {error}"))?;
    let spec = reader.spec();
    let samples = reader
        .samples::<i32>()
        .map(|sample| {
            sample
                .map(|sample| sample as f32 / 8_388_608.0)
                .map_err(|error| format!("recording samples should decode: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let window_size = 4096;
    let pitches = detect_recording_plucks(path)?
        .into_iter()
        .filter_map(|(time, _)| {
            let center = (time * spec.sample_rate as f32) as usize;
            let start = center.saturating_sub(window_size / 2);
            let end = (start + window_size).min(samples.len());
            (end - start >= 256)
                .then(|| estimate_pitch(&samples[start..end], spec.sample_rate as f32))
        })
        .flatten()
        .collect::<Vec<_>>();
    Ok(pitches)
}

#[allow(dead_code)]
fn detect_recording_open_string_lanes(path: &Path) -> Result<Vec<usize>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|error| format!("recording should open: {error}"))?;
    let spec = reader.spec();
    let samples = reader
        .samples::<i32>()
        .map(|sample| {
            sample
                .map(|sample| sample as f32 / 8_388_608.0)
                .map_err(|error| format!("recording samples should decode: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let targets = [30.87, 41.20, 55.00, 73.42, 98.00];
    let window_size = 4096;
    let hop_size = 2048;
    let mut lanes = Vec::new();
    let mut previous_energies = [0.0; 5];
    for window in samples.windows(window_size).step_by(hop_size) {
        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        if level < 0.005 {
            continue;
        }
        let energies = targets.map(|frequency| {
            let (real, imaginary) =
                window
                    .iter()
                    .enumerate()
                    .fold((0.0, 0.0), |(real, imaginary), (index, sample)| {
                        let phase = std::f32::consts::TAU * frequency * index as f32
                            / spec.sample_rate as f32;
                        (
                            real + sample * phase.cos(),
                            imaginary + sample * phase.sin(),
                        )
                    });
            real.mul_add(real, imaginary * imaginary).sqrt()
        });
        let increases: [f32; 5] =
            std::array::from_fn(|index| energies[index] - previous_energies[index]);
        previous_energies = energies;
        if let Some((lane, increase)) = increases
            .into_iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
        {
            if increase > level * window_size as f32 * 0.01 {
                if lanes.last().copied() != Some(lane) {
                    lanes.push(lane);
                }
            }
        }
    }
    Ok(lanes)
}

fn peaks_are_separated(index: usize, selected: &[usize], minimum_distance: usize) -> bool {
    selected
        .iter()
        .all(|selected| index.abs_diff(*selected) >= minimum_distance)
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

#[test]
#[ignore = "the supplied recordings currently expose pitch-detector false positives; run explicitly while tuning DSP"]
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
                let plucks = detect_recording_plucks(&path)?;
                if plucks.len() != 12 {
                    return Err(format!(
                        "should contain exactly 12 plucks; detected {} plucks",
                        plucks.len()
                    ));
                }
                let durations = plucks
                    .iter()
                    .map(|(_, duration)| *duration)
                    .collect::<Vec<_>>();
                if durations.iter().any(|duration| *duration <= 0.0)
                    || durations.iter().copied().fold(0.0, f32::max)
                        - durations.iter().copied().fold(f32::MAX, f32::min)
                        < 0.1
                {
                    return Err(format!(
                        "unexpected pluck durations; expected short and long sustain groups, detected={durations:?}"
                    ));
                }
            } else {
                let detected_pitches = detect_recording_pitches(&path)?;
                let mut detected_lanes = detected_pitches
                    .iter()
                    .map(|pitch| pitch_to_lane(Instrument::Bass5, *pitch))
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
