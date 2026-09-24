pub(crate) mod detector;
pub(crate) use detector::*;

pub(crate) const LANES: usize = 5;

use super::InstrumentKind;

#[derive(Clone, Copy, Debug, PartialEq)]
/// A configured instrument input: which slot it came from, its kind, and (for `Strings`)
/// its string count. Any number of slots of any kind may be configured at once.
pub(crate) struct Instrument {
    pub(crate) slot: usize,
    pub(crate) kind: InstrumentKind,
    pub(crate) strings: u8,
}

#[derive(Clone, Copy, Debug)]
/// Normalized input event sent from an audio or MIDI callback to Bevy.
pub(crate) struct InstrumentEvent {
    pub(crate) instrument: Instrument,
    pub(crate) lane: usize,
    pub(crate) strength: f32,
    pub(crate) pitch_hz: Option<f32>,
    pub(crate) noise_floor: f32,
    pub(crate) phase: NotePhase,
    pub(crate) duration_secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotePhase {
    Started,
    Updated,
    Ended,
}

pub(crate) struct DetectedNote {
    pub(crate) pitch_hz: f32,
    pub(crate) strength: f32,
    pub(crate) noise_floor: f32,
    pub(crate) phase: NotePhase,
    pub(crate) duration_secs: f32,
}

/// Confidence at or above this level (0..1, 1.0 is a perfectly periodic window)
/// is treated as a "confident hit" that can register without a loud attack transient.
pub(crate) const CONFIDENT_PITCH_THRESHOLD: f32 = 0.75;

/// Estimate a fundamental frequency with a normalized YIN-style period search.
pub(crate) fn estimate_pitch(samples: &[f32], sample_rate: f32) -> Option<f32> {
    estimate_pitch_in_range(samples, sample_rate, 30.0, 1400.0)
}

fn estimate_pitch_in_range(
    samples: &[f32],
    sample_rate: f32,
    min_pitch_hz: f32,
    max_pitch_hz: f32,
) -> Option<f32> {
    estimate_pitch_with_confidence(samples, sample_rate, min_pitch_hz, max_pitch_hz)
        .map(|(pitch_hz, _)| pitch_hz)
}

/// Estimate a fundamental frequency along with a 0..1 confidence score, where
/// 1.0 means the window was nearly perfectly periodic at the chosen lag.
pub(crate) fn estimate_pitch_with_confidence(
    samples: &[f32],
    sample_rate: f32,
    min_pitch_hz: f32,
    max_pitch_hz: f32,
) -> Option<(f32, f32)> {
    // Estimate the fundamental period with a normalized difference function.
    if samples.len() < 256 || sample_rate <= 0.0 {
        return None;
    }

    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let centered = samples
        .iter()
        .map(|sample| sample - mean)
        .collect::<Vec<_>>();
    let min_lag = (sample_rate / max_pitch_hz).floor().max(2.0) as usize;
    let max_lag = (sample_rate / min_pitch_hz).ceil() as usize;
    if max_lag >= centered.len() {
        return None;
    }

    // YIN's cumulative difference function finds the first strong period,
    // which avoids mistaking a strong harmonic for the fundamental.
    let mut difference = vec![0.0; max_lag + 1];
    for lag in min_lag..=max_lag {
        difference[lag] = centered[..centered.len() - lag]
            .iter()
            .zip(&centered[lag..])
            .map(|(left, right)| {
                let delta = left - right;
                delta * delta
            })
            .sum();
    }

    // Normalize every lag up front so both the early-exit search and the
    // low-confidence fallback compare on the same (scale-invariant) footing.
    let mut running_sum = 0.0;
    let mut normalized = vec![1.0; max_lag + 1];
    for lag in min_lag..=max_lag {
        running_sum += difference[lag];
        normalized[lag] = difference[lag] * lag as f32 / running_sum.max(f32::EPSILON);
    }

    let mut best_lag = None;
    for lag in min_lag..=max_lag {
        if normalized[lag] < 0.18 && (lag == max_lag || normalized[lag] <= normalized[lag + 1]) {
            best_lag = Some(lag);
            break;
        }
    }
    let lag = best_lag.or_else(|| {
        (min_lag..=max_lag).min_by(|left, right| normalized[*left].total_cmp(&normalized[*right]))
    })?;
    let confidence = (1.0 - normalized[lag]).clamp(0.0, 1.0);

    let refined_lag = if lag > min_lag && lag < max_lag {
        let previous = difference[lag - 1];
        let current = difference[lag];
        let next = difference[lag + 1];
        let denominator = previous - 2.0 * current + next;
        if denominator.abs() > f32::EPSILON {
            lag as f32 + 0.5 * (previous - next) / denominator
        } else {
            lag as f32
        }
    } else {
        lag as f32
    };
    let pitch_hz = sample_rate / refined_lag;
    (min_pitch_hz..=max_pitch_hz)
        .contains(&pitch_hz)
        .then_some((pitch_hz, confidence))
}

/// Convert an instrument pitch into its gameplay lane. `open_frequencies` is the
/// instrument's actual configured tuning (ignored for `Percussion`/`Voice`).
pub(crate) fn pitch_to_lane(
    instrument: Instrument,
    open_frequencies: &[f32],
    pitch_hz: f32,
) -> usize {
    match instrument.kind {
        InstrumentKind::Percussion => 0,
        InstrumentKind::Voice => {
            let (low, high) = (80.0_f32, 1200.0_f32);
            let normalized = ((pitch_hz / low).ln() / (high / low).ln()).clamp(0.0, 0.999);
            (normalized * LANES as f32) as usize
        }
        InstrumentKind::Strings => string_lane(open_frequencies, pitch_hz),
    }
}

/// Map a string-instrument pitch to the nearest lane in its actual configured tuning.
pub(crate) fn string_lane(open_frequencies: &[f32], pitch_hz: f32) -> usize {
    open_frequencies
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (pitch_hz / **left)
                .ln()
                .abs()
                .total_cmp(&(pitch_hz / **right).ln().abs())
        })
        .map_or(0, |(lane, _)| lane.min(LANES - 1))
}
