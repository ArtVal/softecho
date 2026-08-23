//! Общий аудио-пайп перед распознавателем (без UI и без Vosk).
//! Тестируется отдельно — это стык для клиент–сервера / любого ASR.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Целевая частота для Vosk / пайпа.
pub const TARGET_HZ: u32 = 16_000;
/// Один кадр (~100 мс при 16 kHz).
pub const FRAME_SAMPLES: usize = 1_600;
/// Ёмкость буфера перед распознавателем.
pub const PIPE_SECONDS: usize = 120;

pub const fn pipe_capacity_frames() -> usize {
    (TARGET_HZ as usize / FRAME_SAMPLES) * PIPE_SECONDS
}

/// Порог «подождите» (~90% буфера).
pub const fn catch_up_start_frames() -> usize {
    pipe_capacity_frames() * 9 / 10
}

/// FIFO кадров PCM; при `pause` запись отключается.
pub struct AudioPipe {
    q: Mutex<VecDeque<Vec<i16>>>,
    cap: usize,
    pause_input: AtomicBool,
}

impl AudioPipe {
    pub fn new(cap: usize) -> Arc<Self> {
        Arc::new(Self {
            q: Mutex::new(VecDeque::with_capacity(cap)),
            cap,
            pause_input: AtomicBool::new(false),
        })
    }

    pub fn len(&self) -> usize {
        self.q.lock().map(|q| q.len()).unwrap_or(0)
    }

    pub fn set_pause(&self, pause: bool) {
        self.pause_input.store(pause, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.pause_input.load(Ordering::Relaxed)
    }

    pub fn send_frame(&self, frame: Vec<i16>) {
        if self.is_paused() {
            return;
        }
        let Ok(mut q) = self.q.lock() else {
            return;
        };
        if q.len() >= self.cap {
            return;
        }
        q.push_back(frame);
    }

    pub fn try_recv(&self) -> Option<Vec<i16>> {
        self.q.lock().ok()?.pop_front()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<Vec<i16>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(frame) = self.try_recv() {
                return Some(frame);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn drain(&self) -> Vec<Vec<i16>> {
        self.q
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

/// Даунсемпл mono i16 (для 48 kHz → 16 kHz и т.п.).
pub fn downsample_i16(input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if input.is_empty() || from_rate == 0 || to_rate == 0 {
        return Vec::new();
    }
    if from_rate == to_rate {
        return input.to_vec();
    }
    if from_rate.is_multiple_of(to_rate) {
        let factor = (from_rate / to_rate) as usize;
        return input
            .chunks(factor)
            .map(|c| {
                let sum: i32 = c.iter().map(|&s| i32::from(s)).sum();
                (sum / c.len() as i32) as i16
            })
            .collect();
    }
    let out_len = (input.len() as u64 * u64::from(to_rate) / u64::from(from_rate)) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = (i as u64 * u64::from(from_rate) / u64::from(to_rate)) as usize;
        out.push(*input.get(src).unwrap_or(&0));
    }
    out
}

pub fn chunk_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples
        .iter()
        .map(|&s| {
            let v = f64::from(s);
            v * v
        })
        .sum();
    (sum / samples.len() as f64).sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_capacity_is_120_seconds() {
        assert_eq!(pipe_capacity_frames(), 1200);
        assert_eq!(catch_up_start_frames(), 1080);
    }

    #[test]
    fn downsample_48k_to_16k_thirds() {
        let input: Vec<i16> = (0..300).map(|i| i as i16).collect();
        let out = downsample_i16(&input, 48_000, 16_000);
        assert_eq!(out.len(), 100);
        // среднее первых трёх ≈ 1
        assert_eq!(out[0], 1);
    }

    #[test]
    fn downsample_same_rate_copies() {
        let input = vec![1_i16, 2, 3];
        assert_eq!(downsample_i16(&input, 16_000, 16_000), input);
    }

    #[test]
    fn pipe_pause_drops_writes() {
        let pipe = AudioPipe::new(4);
        pipe.send_frame(vec![1, 2]);
        assert_eq!(pipe.len(), 1);
        pipe.set_pause(true);
        pipe.send_frame(vec![3, 4]);
        assert_eq!(pipe.len(), 1);
        pipe.set_pause(false);
        pipe.send_frame(vec![5]);
        assert_eq!(pipe.len(), 2);
    }

    #[test]
    fn pipe_full_does_not_drop_oldest() {
        let pipe = AudioPipe::new(2);
        pipe.send_frame(vec![1]);
        pipe.send_frame(vec![2]);
        pipe.send_frame(vec![3]); // отброшен
        assert_eq!(pipe.len(), 2);
        assert_eq!(pipe.try_recv().unwrap(), vec![1]);
        assert_eq!(pipe.try_recv().unwrap(), vec![2]);
    }

    #[test]
    fn pipe_fifo_and_drain() {
        let pipe = AudioPipe::new(8);
        pipe.send_frame(vec![10]);
        pipe.send_frame(vec![20]);
        assert_eq!(pipe.try_recv().unwrap()[0], 10);
        let rest = pipe.drain();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0][0], 20);
        assert_eq!(pipe.len(), 0);
    }

    #[test]
    fn chunk_rms_silence_and_tone() {
        assert_eq!(chunk_rms(&[]), 0.0);
        assert!(chunk_rms(&[0, 0, 0]) < 1.0);
        assert!(chunk_rms(&[10_000, -10_000, 10_000]) > 1000.0);
    }
}
