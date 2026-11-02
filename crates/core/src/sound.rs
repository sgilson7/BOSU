//! Sounds the game makes itself, as WAV bytes the page decodes. The
//! calibration click: 30 ms of a 1 kHz tone that dies away.

use crate::fx;

pub const RATE: u32 = 44_100;
const CLICK_SAMPLES: u32 = RATE * 30 / 1000;
const CLICK_HZ: u32 = 1000;

/// A mono 16-bit WAV file of `samples`.
pub fn wav(samples: &[i16]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(RATE.to_le_bytes());
    out.extend((RATE * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    for s in samples {
        out.extend(s.to_le_bytes());
    }
    out
}

pub fn click_samples() -> Vec<i16> {
    (0..CLICK_SAMPLES)
        .map(|k| {
            // the phase in turns of fx::TURN, wrapping each cycle
            let phase = ((k as u64 * CLICK_HZ as u64 * fx::TURN as u64) / RATE as u64) as i32 % fx::TURN;
            let level = 24_000 * (CLICK_SAMPLES - k) as i64 / CLICK_SAMPLES as i64;
            ((fx::sin(phase) as i64 * level) >> fx::SHIFT) as i16
        })
        .collect()
}

pub fn click_wav() -> Vec<u8> {
    wav(&click_samples())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_header_names_its_format_and_length() {
        let w = wav(&[1, -1]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([w[4], w[5], w[6], w[7]]), 40);
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), RATE);
        assert_eq!(&w[36..40], b"data");
        assert_eq!(&w[44..48], &[1, 0, 0xff, 0xff]);
    }

    #[test]
    fn the_click_starts_loud_and_dies_away() {
        let s = click_samples();
        assert_eq!(s.len() as u32, CLICK_SAMPLES);
        let peak = |r: std::ops::Range<usize>| s[r].iter().map(|v| v.unsigned_abs()).max().unwrap();
        assert!(peak(0..100) > 15_000);
        assert!(peak(s.len() - 100..s.len()) < 2_000);
    }
}
