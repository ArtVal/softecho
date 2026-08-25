//! Воспроизведение последней записи (mono i16 @ 16 kHz).
//! Реальный вывод — только при feature = "asr" (cpal).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Частота захвата ASR / playback (как у Vosk).
#[allow(dead_code)] // используется в asr (feature) и в cpal_play
pub const PLAYBACK_HZ: u32 = 16_000;

/// Максимум хвоста записи для «Послушать» (~60 с @ 16 kHz).
#[allow(dead_code)] // используется в asr (feature)
pub const CAPTURE_MAX_SAMPLES: usize = PLAYBACK_HZ as usize * 60;

/// Проиграть PCM в фоне. `stop` — досрочная остановка; по концу сбрасывает `busy`.
pub fn play_pcm_16k(samples: Vec<i16>, stop: Arc<AtomicBool>, busy: Arc<AtomicBool>) {
    thread::spawn(move || {
        busy.store(true, Ordering::Relaxed);
        let _ = play_blocking(&samples, &stop);
        busy.store(false, Ordering::Relaxed);
        stop.store(false, Ordering::Relaxed);
    });
}

fn play_blocking(samples: &[i16], stop: &Arc<AtomicBool>) -> Result<(), String> {
    if samples.is_empty() {
        return Ok(());
    }
    #[cfg(feature = "asr")]
    {
        cpal_play(samples, stop)
    }
    #[cfg(not(feature = "asr"))]
    {
        let _ = (samples, stop);
        Err("Воспроизведение недоступно без ASR".into())
    }
}

#[cfg(feature = "asr")]
fn cpal_play(samples: &[i16], stop: &Arc<AtomicBool>) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::Mutex;

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "Динамик не найден".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("Настройка звука: {e}"))?;

    let out_hz = config.sample_rate().0;
    let channels = config.channels() as usize;
    let pcm = Arc::new(resample_i16_to_f32(samples, PLAYBACK_HZ, out_hz));
    let cursor = Arc::new(Mutex::new(0usize));
    let done = Arc::new(AtomicBool::new(false));
    let err_fn = |err| eprintln!("Ошибка воспроизведения: {err}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let pcm = Arc::clone(&pcm);
            let cursor = Arc::clone(&cursor);
            let done = Arc::clone(&done);
            let stop_flag = Arc::clone(stop);
            device
                .build_output_stream(
                    &config.clone().into(),
                    move |data: &mut [f32], _| {
                        fill_f32(data, channels, &pcm, &cursor, &done, &stop_flag);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Поток звука: {e}"))?
        }
        cpal::SampleFormat::I16 => {
            let pcm = Arc::clone(&pcm);
            let cursor = Arc::clone(&cursor);
            let done = Arc::clone(&done);
            let stop_flag = Arc::clone(stop);
            device
                .build_output_stream(
                    &config.clone().into(),
                    move |data: &mut [i16], _| {
                        if stop_flag.load(Ordering::Relaxed) {
                            data.fill(0);
                            done.store(true, Ordering::Relaxed);
                            return;
                        }
                        let Ok(mut pos) = cursor.lock() else {
                            done.store(true, Ordering::Relaxed);
                            return;
                        };
                        let mut i = 0;
                        while i < data.len() {
                            if *pos >= pcm.len() {
                                data[i..].fill(0);
                                done.store(true, Ordering::Relaxed);
                                return;
                            }
                            let sample = (pcm[*pos].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            *pos += 1;
                            for _ in 0..channels {
                                if i < data.len() {
                                    data[i] = sample;
                                    i += 1;
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Поток звука: {e}"))?
        }
        other => {
            return Err(format!("Формат динамика не поддерживается: {other:?}"));
        }
    };

    stream.play().map_err(|e| format!("Старт звука: {e}"))?;

    while !done.load(Ordering::Relaxed) {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(20));
    }
    drop(stream);
    Ok(())
}

#[cfg(feature = "asr")]
fn fill_f32(
    data: &mut [f32],
    channels: usize,
    pcm: &[f32],
    cursor: &std::sync::Mutex<usize>,
    done: &AtomicBool,
    stop_flag: &AtomicBool,
) {
    if stop_flag.load(Ordering::Relaxed) {
        data.fill(0.0);
        done.store(true, Ordering::Relaxed);
        return;
    }
    let Ok(mut pos) = cursor.lock() else {
        done.store(true, Ordering::Relaxed);
        return;
    };
    let mut i = 0;
    while i < data.len() {
        if *pos >= pcm.len() {
            data[i..].fill(0.0);
            done.store(true, Ordering::Relaxed);
            return;
        }
        let sample = pcm[*pos];
        *pos += 1;
        for _ in 0..channels {
            if i < data.len() {
                data[i] = sample;
                i += 1;
            }
        }
    }
}

fn resample_i16_to_f32(input: &[i16], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if input.is_empty() || from_hz == 0 || to_hz == 0 {
        return Vec::new();
    }
    if from_hz == to_hz {
        return input
            .iter()
            .map(|&s| s as f32 / i16::MAX as f32)
            .collect();
    }
    let out_len = (input.len() as u64 * u64::from(to_hz) / u64::from(from_hz)) as usize;
    let mut out = Vec::with_capacity(out_len.max(1));
    for i in 0..out_len {
        let src_f = i as f64 * f64::from(from_hz) / f64::from(to_hz);
        let i0 = src_f.floor() as usize;
        let i1 = (i0 + 1).min(input.len().saturating_sub(1));
        let t = (src_f - i0 as f64) as f32;
        let a = input.get(i0).copied().unwrap_or(0) as f32;
        let b = input.get(i1).copied().unwrap_or(0) as f32;
        let s = a + (b - a) * t;
        out.push(s / i16::MAX as f32);
    }
    out
}

/// Дописать кадр в кольцевой захват (хвост не длиннее `max_samples`).
pub fn push_capture(buf: &mut Vec<i16>, frame: &[i16], max_samples: usize) {
    if frame.is_empty() || max_samples == 0 {
        return;
    }
    buf.extend_from_slice(frame);
    if buf.len() > max_samples {
        let drop_n = buf.len() - max_samples;
        buf.drain(..drop_n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_same_rate_keeps_length() {
        let input = vec![0_i16, 1000, -1000];
        let out = resample_i16_to_f32(&input, 16_000, 16_000);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn resample_up_roughly_doubles() {
        let input = vec![0_i16; 100];
        let out = resample_i16_to_f32(&input, 16_000, 32_000);
        assert!((190..=210).contains(&out.len()));
    }

    #[test]
    fn capture_ring_keeps_tail() {
        let mut buf = Vec::new();
        push_capture(&mut buf, &[1, 2, 3, 4, 5], 3);
        assert_eq!(buf, vec![3, 4, 5]);
        push_capture(&mut buf, &[6, 7], 3);
        assert_eq!(buf, vec![5, 6, 7]);
    }
}
