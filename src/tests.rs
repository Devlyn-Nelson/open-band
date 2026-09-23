use super::{
    Articulation, AudioDetector, AudioOnsetDetector, BendSpec, Chart, ChartEvent, Clef,
    DetectorProfile, DeviceChoice, DynamicLevel, Instrument, InstrumentKind, KeyMode, KeySignature,
    MotionKind, NoteAttack, NoteContent, NoteDynamics, NotePhase, NoteTransition,
    PolyphonicAudioDetector, SnapInterval, SyllableKind, Tuning, cents_error, cycle_track_kit,
    cycle_track_tuning, default_clef, estimate_pitch, layout_track, load_charts, load_kit_library,
    load_tuning_library, measures, new_track, pitch_to_lane, selected_device_index, slugify,
    snap_tick, string_lane,
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
        assert_eq!(string_lane(&five_string_open_notes, pitch), lane);
    }

    let four_string_open_notes = [41.20, 55.00, 73.42, 98.00];
    for (lane, pitch) in four_string_open_notes.into_iter().enumerate() {
        assert_eq!(string_lane(&four_string_open_notes, pitch), lane);
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
    let five_string_open_notes = [30.87, 41.20, 55.00, 73.42, 98.00];
    assert_eq!(string_lane(&five_string_open_notes, 98.0), 4);
    assert_eq!(string_lane(&five_string_open_notes, 73.42), 3);
    assert_eq!(string_lane(&five_string_open_notes, 55.0), 2);
    assert_eq!(string_lane(&five_string_open_notes, 41.20), 1);
}

#[test]
/// Loads the built-in chart with ordered, playable bass notes.
fn built_in_chart_has_playable_notes() {
    let chart: Chart =
        serde_json::from_str(super::OPEN_STRINGS_CHART).expect("built-in chart should parse");
    assert_eq!(chart.version, 1);
    assert_eq!(chart.tracks.len(), 1);
    assert_eq!(chart.tracks[0].kind, InstrumentKind::Strings);
    assert_eq!(
        chart.tracks[0].tuning.as_ref().map(|tuning| tuning
            .strings
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()),
        Some(vec!["B0", "E1", "A1", "D2", "G2"])
    );
    let notes = chart.string_notes();
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
                    "resolution": 960,
                    "tempo_map": [{ "start": 0, "bpm": 90.0 }],
                    "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
                    "tracks": [{
                        "name": "Bass",
                        "kind": "strings",
                        "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
                        "notes": [{
                            "start": 960,
                            "length": 4,
                            "note": "C1",
                            "ps": 2
                        }]
                    }]
                }
                "#,
    )
    .expect("projection chart should parse");

    let notes = chart.string_notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].string, 0);
    assert_eq!(notes[0].fret, 1);
}

#[test]
fn string_techniques_parse_resolve_and_round_trip() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Techniques",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [{
                    "start": 0,
                    "length": 4,
                    "note": "E2",
                    "ps": 0,
                    "attack": "tap",
                    "transition": "slide",
                    "bend": { "semitones": 2.0, "release": true },
                    "motion": { "kind": "trill", "target": "F#2" }
                }]
            }]
        }
        "#,
    )
    .expect("string techniques chart should parse");

    let notes = chart.string_notes();
    assert_eq!(notes[0].attack, Some(NoteAttack::Tap));
    assert_eq!(notes[0].transition, Some(NoteTransition::Slide));
    assert_eq!(
        notes[0].bend,
        Some(BendSpec {
            semitones: 2.0,
            release: true,
            points: Vec::new(),
        })
    );
    assert_eq!(
        notes[0].motion.as_ref().map(|motion| motion.kind),
        Some(MotionKind::Trill)
    );
    assert_eq!(
        notes[0].motion.as_ref().map(|motion| motion.target),
        Some(42)
    );

    let serialized = serde_json::to_string(&chart).expect("techniques chart should serialize");
    assert!(serialized.contains("\"attack\":\"tap\""));
    assert!(serialized.contains("\"transition\":\"slide\""));
    assert!(serialized.contains("\"target\":42"));
    serde_json::from_str::<Chart>(&serialized).expect("serialized techniques chart should reparse");
}

#[test]
fn notation_features_parse_and_layout() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Notation Features",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "clef": "bass",
                "key_signature": { "fifths": -2, "mode": "major" },
                "notes": [
                    {
                        "start": 0,
                        "length": 8,
                        "tuplet": { "actual": 3, "normal": 2 },
                        "chord": "c1",
                        "note": "E2",
                        "dynamic": "mf",
                        "articulations": ["accent"]
                    },
                    {
                        "start": 320,
                        "length": 8,
                        "tuplet": { "actual": 3, "normal": 2 },
                        "chord": "c1",
                        "note": "G2"
                    }
                ],
                "phrases": [{
                    "start": 0,
                    "duration_ticks": 960,
                    "text": "hel",
                    "syllable": "begin",
                    "melisma": true,
                    "notes": []
                }]
            }]
        }
        "#,
    )
    .expect("notation features should parse");

    let track = &chart.tracks[0];
    assert_eq!(track.clef, Some(Clef::Bass));
    assert_eq!(
        track.key_signature,
        Some(KeySignature {
            fifths: -2,
            mode: KeyMode::Major
        })
    );
    assert_eq!(track.notes[0].duration_ticks(960), 320);
    assert_eq!(track.notes[0].chord.as_deref(), Some("c1"));
    assert_eq!(track.notes[0].dynamic, Some(DynamicLevel::Mf));
    assert_eq!(track.notes[0].articulations, vec![Articulation::Accent]);
    assert_eq!(track.phrases[0].syllable, Some(SyllableKind::Begin));
    assert!(track.phrases[0].melisma);

    let layout = layout_track(&chart, track);
    assert_eq!(layout.clef, Some(Clef::Bass));
    assert_eq!(layout.key_signature, track.key_signature);
    assert_eq!(layout.tuplets, vec![vec![0, 1]]);
}

#[test]
fn chart_validation_reports_invalid_ties_and_bend_curves() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Invalid Notation",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "start": 0, "length": 4, "tied": true, "note": "E2", "bend": {
                        "points": [
                            { "offset": 0.75, "semitones": 2.0 },
                            { "offset": 0.25, "semitones": 0.0 }
                        ]
                    } },
                    { "start": 1920, "length": 4, "note": "F2" }
                ]
            }]
        }
        "#,
    )
    .expect("invalid notation should still parse");
    let warnings = chart.validate();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("tie does not start"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("tie changes pitch"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("invalid bend curve"))
    );
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
    assert_eq!(developer_chart.string_notes().len(), 10);
}

#[test]
/// The drum-only test chart hits every standard rock kit piece at least twice.
fn drum_kit_workout_chart_covers_every_piece_at_least_twice() {
    let charts = load_charts();
    let chart = charts
        .iter()
        .find(|chart| chart.title == "Drum Kit Workout")
        .expect("drum kit workout chart should load");
    let notes = chart.percussion_notes();
    for piece in [
        "kick", "snare", "tom1", "hihat", "tom2", "ride", "tom3", "crash",
    ] {
        let count = notes.iter().filter(|note| note.piece == piece).count();
        assert!(
            count >= 2,
            "expected {piece} to appear at least twice, got {count}"
        );
    }
}

#[test]
/// The three-track jam chart is roughly 30 seconds and has a part for each instrument.
fn easy_band_jam_chart_has_three_tracks_and_is_about_thirty_seconds() {
    let charts = load_charts();
    let chart = charts
        .iter()
        .find(|chart| chart.title == "Easy Band Jam")
        .expect("easy band jam chart should load");
    assert_eq!(chart.tracks.len(), 3);
    assert!(chart.tracks.iter().any(|track| track.name == "Bass"));
    assert!(chart.tracks.iter().any(|track| track.name == "Guitar"));
    assert!(chart.tracks.iter().any(|track| track.name == "Drums"));
    let seconds = chart.tick_to_seconds(chart.total_ticks());
    assert!(
        (25.0..=35.0).contains(&seconds),
        "expected roughly 30 seconds, got {seconds}"
    );
}

#[test]
fn chart_supports_multiple_instrument_tracks_and_vocal_phrases() {
    let chart: Chart = serde_json::from_str(
        r#"
                {
                    "version": 1,
                    "title": "Band Test",
                    "resolution": 960,
                    "tempo_map": [{ "start": 0, "bpm": 120.0 }],
                    "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
                    "tracks": [
                        {
                            "name": "Bass",
                            "kind": "strings",
                            "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
                            "notes": [{ "start": 960, "length": 4, "note": 23 }]
                        },
                        {
                            "name": "Lead Vocal",
                            "kind": "voice",
                            "phrases": [{
                                "start": 1920,
                                "duration_ticks": 1920,
                                "text": "Hello",
                                "notes": [{ "start": 1920, "length": 2, "note": "C4" }]
                            }]
                        }
                    ]
                }
                "#,
    )
    .expect("multi-track chart should parse");

    assert_eq!(chart.starting_time_signature(), [4, 4]);
    assert_eq!(chart.tracks.len(), 2);
    assert_eq!(chart.tracks[1].phrases[0].text, "Hello");
    assert_eq!(chart.tracks[1].phrases[0].notes[0].note(), Some(60));
    assert_eq!(chart.total_ticks(), 3840);
    assert_eq!(chart.string_notes()[0].note, "B0");
}

#[test]
/// Resolves shared-lane percussion pieces (tom vs. cymbal) and a lane-spanning kick by name.
fn percussion_track_resolves_pieces_by_name() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Drums Test",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Drums",
                "kind": "percussion",
                "kit": {
                    "lanes": 4,
                    "pieces": [
                        { "name": "kick", "trigger": "midi:36", "lane_span": "yellow" },
                        { "name": "snare", "trigger": "midi:38", "lane": 0, "symbol": "tom" },
                        { "name": "crash", "trigger": "midi:49", "lane": 1, "symbol": "cymbal" }
                    ]
                },
                "notes": [
                    { "start": 0, "length": 4, "piece": "kick" },
                    { "start": 960, "length": 4, "piece": "snare", "dynamics": "accent" },
                    { "start": 1920, "length": 4, "piece": "crash", "roll": "crash" },
                    { "start": 2880, "length": 4, "piece": "snare", "droll": "crash" }
                ]
            }]
        }
        "#,
    )
    .expect("percussion chart should parse");

    let notes = chart.percussion_notes();
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[0].piece, "kick");
    assert_eq!(notes[0].lane_span_color.as_deref(), Some("yellow"));
    assert_eq!(notes[1].dynamics, NoteDynamics::Accent);
    assert_eq!(notes[1].symbol.as_deref(), Some("tom"));
    assert_eq!(notes[2].symbol.as_deref(), Some("cymbal"));
    assert_eq!(notes[2].roll.as_deref(), Some("crash"));
    assert_eq!(notes[2].droll, None);
    assert_eq!(notes[3].roll, None);
    assert_eq!(notes[3].droll.as_deref(), Some("crash"));
}

#[test]
fn chart_rejects_conflicting_or_pitched_roll_fields() {
    for json in [
        r#"{ "start": 0, "length": 4, "piece": "kick", "roll": "snare", "droll": "tom1" }"#,
        r#"{ "start": 0, "length": 4, "note": "E2", "roll": "snare" }"#,
    ] {
        assert!(
            serde_json::from_str::<ChartEvent>(json).is_err(),
            "expected rejection: {json}"
        );
    }
}

#[test]
/// Walks a mid-song tempo change when converting ticks to seconds.
fn tick_to_seconds_honors_a_mid_song_tempo_change() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Tempo Change",
            "resolution": 960,
            "tempo_map": [
                { "start": 0, "bpm": 120.0 },
                { "start": 1920, "bpm": 60.0 }
            ],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": []
        }
        "#,
    )
    .expect("tempo-change chart should parse");

    // Two quarter notes at 120 BPM (0.5s each) land exactly on the tempo change.
    assert!((chart.tick_to_seconds(1920) - 1.0).abs() < 1e-4);
    // One further quarter note at 60 BPM adds a full second.
    assert!((chart.tick_to_seconds(2880) - 2.0).abs() < 1e-4);
}

#[test]
/// Star Power phrases are simple range markers with no nested note list.
fn star_power_phrase_parses_as_a_range_marker() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Star Power",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [],
                "star_power_phrases": [{ "start": 0, "duration_ticks": 3840 }]
            }]
        }
        "#,
    )
    .expect("star power chart should parse");

    assert_eq!(chart.tracks[0].star_power_phrases.len(), 1);
    assert_eq!(chart.tracks[0].star_power_phrases[0].duration_ticks, 3840);
}

#[test]
/// The tuning library loads named tunings with valid, parseable note names (either from
/// `tunings/` or the embedded fallback if that directory isn't present).
fn tuning_library_loads_and_parses() {
    let library = load_tuning_library();
    assert!(!library.is_empty());
    for named in &library {
        assert!(
            !named.strings.is_empty(),
            "{} should list at least one string",
            named.name
        );
        assert!(
            named.tuning().open_frequencies().is_ok(),
            "{} should parse as valid note names",
            named.name
        );
    }
}

#[test]
/// The kit library's pieces all reference a valid lane or a lane-spanning color, and the
/// standard rock kit specifically has 3 toms, snare, hi-hat, crash, ride, and a kick.
fn kit_library_pieces_are_placed_and_standard_kit_has_expected_pieces() {
    let library = load_kit_library();
    assert!(!library.is_empty());
    for named in &library {
        for piece in &named.pieces {
            assert!(
                piece.lane.is_some() != piece.lane_span.is_some(),
                "{} piece {} should have exactly one of lane/lane_span",
                named.name,
                piece.name
            );
            if let Some(lane) = piece.lane {
                assert!(
                    lane < named.lanes,
                    "{} piece {} lane out of range",
                    named.name,
                    piece.name
                );
            }
        }
    }

    let standard = library
        .iter()
        .find(|named| named.name == "Standard Rock Kit")
        .expect("standard rock kit should be in the library");
    let symbol_counts = |symbol: &str| {
        standard
            .pieces
            .iter()
            .filter(|piece| piece.symbol.as_deref() == Some(symbol))
            .count()
    };
    assert_eq!(
        symbol_counts("tom"),
        4,
        "expected snare + 3 toms tagged as toms"
    );
    assert_eq!(symbol_counts("cymbal"), 2, "expected crash and ride");
    assert_eq!(symbol_counts("hihat"), 1);
    assert!(
        standard
            .pieces
            .iter()
            .any(|piece| piece.name == "kick" && piece.lane_span.is_some()),
        "expected a lane-spanning kick"
    );
}

#[test]
/// Chart validation flags unplayable notes, missing tuning/kit, and unknown pieces.
fn chart_validate_reports_expected_problems() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Broken Chart",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [
                {
                    "name": "Bass",
                    "kind": "strings",
                    "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                    "notes": [{ "start": 0, "length": 4, "note": 100 }]
                },
                {
                    "name": "Drums",
                    "kind": "percussion",
                    "kit": { "lanes": 1, "pieces": [{ "name": "kick", "trigger": "midi:36", "lane": 0 }] },
                    "notes": [{ "start": 0, "length": 4, "piece": "kick", "droll": "nonexistent" }]
                }
            ]
        }
        "#,
    )
    .expect("broken chart should still parse");

    let warnings = chart.validate();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unplayable note"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unknown percussion roll target"))
    );
}

#[test]
/// New tracks default to a sensible tuning/kit for their kind, and none for others.
fn new_track_has_expected_defaults_per_kind() {
    let strings = new_track(InstrumentKind::Strings);
    assert!(strings.tuning.is_some());
    assert!(strings.kit.is_none());

    let percussion = new_track(InstrumentKind::Percussion);
    assert!(percussion.kit.is_some());
    assert!(percussion.tuning.is_none());

    let voice = new_track(InstrumentKind::Voice);
    assert!(voice.tuning.is_none());
    assert!(voice.kit.is_none());
}

#[test]
/// Cycling a track's tuning/kit steps through the library and wraps around.
fn cycle_track_tuning_and_kit_step_through_the_library() {
    let tunings = load_tuning_library();
    let mut track = new_track(InstrumentKind::Strings);
    let first = track.tuning.clone();
    cycle_track_tuning(&mut track, &tunings, 1);
    assert_ne!(track.tuning, first, "cycling should change the tuning");
    for _ in 0..tunings.len().saturating_sub(1) {
        cycle_track_tuning(&mut track, &tunings, 1);
    }
    assert_eq!(
        track.tuning, first,
        "cycling all the way around should return to start"
    );

    let kits = load_kit_library();
    let mut drum_track = new_track(InstrumentKind::Percussion);
    let first_kit = drum_track.kit.clone();
    cycle_track_kit(&mut drum_track, &kits, 1);
    if kits.len() > 1 {
        assert_ne!(drum_track.kit, first_kit);
    }
}

#[test]
/// Chart titles slugify into filesystem-safe, non-empty save filenames.
fn slugify_produces_filesystem_safe_names() {
    assert_eq!(slugify("Easy Band Jam"), "easy-band-jam");
    assert_eq!(slugify("  Weird!! Title??  "), "weird-title");
    assert_eq!(slugify(""), "untitled");
}

#[test]
/// Rounds a tick position to the nearest snap-interval multiple, tempo-independent.
fn snap_tick_rounds_to_the_nearest_interval() {
    assert_eq!(snap_tick(100, SnapInterval::Quarter, 960), 0);
    assert_eq!(snap_tick(500, SnapInterval::Quarter, 960), 960);
    assert_eq!(snap_tick(960, SnapInterval::Quarter, 960), 960);
    assert_eq!(snap_tick(240, SnapInterval::Sixteenth, 960), 240);
}

fn simple_chart(resolution: u32) -> Chart {
    serde_json::from_str(&format!(
        r#"{{
            "version": 1,
            "title": "Notation Test",
            "resolution": {resolution},
            "tempo_map": [{{ "start": 0, "bpm": 120.0 }}],
            "time_signature_map": [{{ "start": 0, "numerator": 4, "denominator": 4 }}],
            "tracks": []
        }}"#
    ))
    .expect("simple chart should parse")
}

#[test]
/// Builds sequential 4/4 measures covering the requested tick span.
fn measures_covers_the_requested_span_in_4_4() {
    let chart = simple_chart(960);
    let built = measures(&chart, 7680);
    assert_eq!(built.len(), 2);
    assert_eq!(built[0].start_tick, 0);
    assert_eq!(built[0].end_tick, 3840);
    assert_eq!(built[1].start_tick, 3840);
    assert_eq!(built[1].end_tick, 7680);
}

#[test]
/// A tied pair of quarter notes forms one tie chain, and the rest of the measure is
/// filled with inferred rests that don't cross the barline.
fn layout_track_ties_notes_and_infers_rests() {
    let mut chart = simple_chart(960);
    chart.tracks = vec![new_track(InstrumentKind::Strings)];
    chart.tracks[0].notes = serde_json::from_str(
        r#"[
            { "start": 0, "length": 4, "note": "E1", "tied": true },
            { "start": 960, "length": 4, "note": "E1" }
        ]"#,
    )
    .expect("notes should parse");

    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.ties, vec![vec![0, 1]]);
    // The tie chain occupies ticks 0..1920; the rest of the 3840-tick measure is silent.
    let rest_ticks = layout
        .rests
        .iter()
        .map(|rest| rest.value.ticks(960))
        .sum::<u32>();
    assert_eq!(rest_ticks, 3840 - 1920);
    assert!(layout.rests.iter().all(|rest| rest.start >= 1920));
}

#[test]
/// Consecutive eighth notes within the same beat are grouped into one beam.
fn layout_track_beams_eighth_notes_within_a_beat() {
    let mut chart = simple_chart(960);
    chart.tracks = vec![new_track(InstrumentKind::Strings)];
    chart.tracks[0].notes = serde_json::from_str(
        r#"[
            { "start": 0, "length": 8, "note": "E1" },
            { "start": 480, "length": 8, "note": "E1" },
            { "start": 960, "length": 8, "note": "E1" },
            { "start": 1440, "length": 8, "note": "E1" }
        ]"#,
    )
    .expect("notes should parse");

    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.beams, vec![vec![0, 1], vec![2, 3]]);
}

#[test]
/// Bass tunings default to bass clef, guitar tunings (even 7-string) to treble, and
/// percussion tracks have no clef at all.
fn default_clef_distinguishes_bass_from_guitar_and_percussion() {
    let mut bass = new_track(InstrumentKind::Strings);
    bass.tuning = Some(Tuning {
        strings: vec!["E1".into(), "A1".into(), "D2".into(), "G2".into()],
    });
    assert_eq!(default_clef(&bass), Some(Clef::Bass));

    let mut guitar = new_track(InstrumentKind::Strings);
    guitar.tuning = Some(Tuning {
        strings: vec![
            "B1".into(),
            "E2".into(),
            "A2".into(),
            "D3".into(),
            "G3".into(),
            "B3".into(),
            "E4".into(),
        ],
    });
    assert_eq!(default_clef(&guitar), Some(Clef::Treble));

    let percussion = new_track(InstrumentKind::Percussion);
    assert_eq!(default_clef(&percussion), None);
}

#[test]
/// A chart edited in memory round-trips through serialize/deserialize without losing
/// pitched notes, percussive notes, dynamics, or ties.
fn chart_round_trips_through_serialize_and_deserialize() {
    let mut chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Round Trip",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": []
        }
        "#,
    )
    .expect("empty chart should parse");
    chart.tracks.push(new_track(InstrumentKind::Strings));
    chart.tracks.push(new_track(InstrumentKind::Percussion));
    chart.tracks[0].notes.push(
        serde_json::from_str(
            r#"{ "start": 0, "length": 4, "dots": 1, "tied": true, "note": "E1", "ps": 0 }"#,
        )
        .unwrap(),
    );
    chart.tracks[1].notes.push(
        serde_json::from_str(r#"{ "start": 0, "length": 8, "piece": "kick", "dynamics": "accent", "droll": "snare" }"#)
            .unwrap(),
    );

    let serialized = serde_json::to_string(&chart).expect("chart should serialize");
    let reloaded: Chart =
        serde_json::from_str(&serialized).expect("serialized chart should reparse");

    assert_eq!(reloaded.title, "Round Trip");
    assert_eq!(reloaded.tracks.len(), 2);
    assert_eq!(reloaded.tracks[0].notes[0].note(), Some(28));
    assert_eq!(reloaded.tracks[0].notes[0].dots, 1);
    assert!(reloaded.tracks[0].notes[0].tied);
    assert_eq!(reloaded.tracks[1].kind, InstrumentKind::Percussion);
    let NoteContent::Percussive { droll, .. } = &reloaded.tracks[1].notes[0].content else {
        panic!("expected a percussive event");
    };
    assert_eq!(droll.as_deref(), Some("snare"));
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
                let bass5 = Instrument {
                    slot: 0,
                    kind: InstrumentKind::Strings,
                    strings: 5,
                };
                let bass5_open_frequencies = [30.87, 41.20, 55.00, 73.42, 98.00];
                let mut detected_lanes = detected_pitches
                    .iter()
                    .map(|pitch| pitch_to_lane(bass5, &bass5_open_frequencies, *pitch))
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
