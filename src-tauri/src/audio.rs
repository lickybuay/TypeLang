//! Microphone capture for dictation.
//!
//! cpal's `Stream` is `!Send` on macOS, so a dedicated thread owns it for its
//! whole life; the rest of the app only holds a `Sender<RecorderCmd>` (see
//! `AppState`). The thread is started lazily on the first recording. Audio is
//! downmixed to mono at the device's native sample rate and encoded as a
//! 16-bit WAV in memory — no resampling, the STT server converts on its end.

use std::io::Cursor;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use tokio::sync::oneshot;

use crate::state::AppState;

/// Hard cap on a single recording. The popup also auto-stops at this length;
/// this bound only keeps memory finite if it ever doesn't.
pub const MAX_RECORDING_SECS: u32 = 120;
/// Shorter recordings are accidental clicks: dropped without an error.
const MIN_RECORDING_SECS: f32 = 0.3;
/// A recording nobody stopped after this long past the cap is dropped (the
/// popup is gone), so its samples don't linger.
const ABANDONED_GRACE_SECS: u64 = 30;
const WATCHDOG_TICK: Duration = Duration::from_millis(250);

pub enum RecorderCmd {
    Start(oneshot::Sender<Result<(), String>>),
    /// Replies with the WAV bytes, or an empty `Vec` for a recording too
    /// short to keep.
    Stop(oneshot::Sender<Result<Vec<u8>, String>>),
    Cancel,
}

#[derive(Default)]
struct Shared {
    samples: Mutex<Vec<f32>>,
    error: Mutex<Option<String>>,
}

struct Active {
    /// `None` once the cap is reached: capture stops (mic released) but the
    /// samples stay available for a later Stop.
    stream: Option<cpal::Stream>,
    shared: Arc<Shared>,
    sample_rate: u32,
    started: Instant,
}

fn spawn_thread() -> Sender<RecorderCmd> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run(rx));
    tx
}

fn sender(state: &AppState) -> Result<Sender<RecorderCmd>, String> {
    let mut slot = state.recorder.lock().map_err(|e| e.to_string())?;
    Ok(slot.get_or_insert_with(spawn_thread).clone())
}

fn run(rx: Receiver<RecorderCmd>) {
    let mut active: Option<Active> = None;
    loop {
        let cmd = match rx.recv_timeout(WATCHDOG_TICK) {
            Ok(cmd) => cmd,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                enforce_cap(&mut active);
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match cmd {
            RecorderCmd::Start(reply) => {
                // A start while one is running restarts from scratch.
                active = None;
                let result = begin().map(|a| active = Some(a));
                let _ = reply.send(result);
            }
            RecorderCmd::Stop(reply) => {
                let result = match active.take() {
                    Some(a) => finish(a),
                    None => Err("No hay una grabación en curso".to_string()),
                };
                let _ = reply.send(result);
            }
            RecorderCmd::Cancel => {
                if active.take().is_some() {
                    println!("[audio] recording cancelled");
                }
            }
        }
    }
}

fn enforce_cap(active: &mut Option<Active>) {
    let Some(a) = active.as_mut() else {
        return;
    };
    let elapsed = a.started.elapsed().as_secs();
    if elapsed >= u64::from(MAX_RECORDING_SECS) + ABANDONED_GRACE_SECS {
        println!("[audio] abandoned recording dropped");
        *active = None;
    } else if elapsed >= u64::from(MAX_RECORDING_SECS) && a.stream.take().is_some() {
        println!("[audio] recording hit the {MAX_RECORDING_SECS}s cap, capture stopped");
    }
}

fn begin() -> Result<Active, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No se encontró ningún micrófono")?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("No se pudo acceder al micrófono: {e}"))?;
    let format = supported.sample_format();
    let sample_rate = supported.sample_rate().0;
    let config = supported.config();
    let shared = Arc::new(Shared::default());

    let built = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, &shared),
        SampleFormat::I16 => build::<i16>(&device, &config, &shared),
        SampleFormat::U16 => build::<u16>(&device, &config, &shared),
        SampleFormat::I32 => build::<i32>(&device, &config, &shared),
        other => return Err(format!("Formato de audio no soportado: {other:?}")),
    };
    let stream = built.map_err(|e| format!("No se pudo acceder al micrófono: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("No se pudo iniciar la grabación: {e}"))?;

    println!("[audio] recording started ({sample_rate} Hz, {format:?})");
    Ok(Active {
        stream: Some(stream),
        shared,
        sample_rate,
        started: Instant::now(),
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = usize::from(config.channels.max(1));
    let cap = (config.sample_rate.0 * MAX_RECORDING_SECS) as usize;
    let data_shared = Arc::clone(shared);
    let err_shared = Arc::clone(shared);
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let Ok(mut buf) = data_shared.samples.lock() else {
                return;
            };
            for frame in data.chunks(channels) {
                if buf.len() >= cap {
                    break;
                }
                let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
                buf.push(sum / channels as f32);
            }
        },
        move |err| {
            eprintln!("[audio] stream error: {err}");
            if let Ok(mut slot) = err_shared.error.lock() {
                *slot = Some(err.to_string());
            }
        },
        None,
    )
}

fn finish(active: Active) -> Result<Vec<u8>, String> {
    let Active {
        stream,
        shared,
        sample_rate,
        ..
    } = active;
    drop(stream);

    let samples = std::mem::take(&mut *shared.samples.lock().map_err(|e| e.to_string())?);
    if samples.is_empty() {
        let detail = shared.error.lock().ok().and_then(|e| e.clone());
        return match detail {
            Some(e) => Err(format!("Error del micrófono: {e}")),
            None => Ok(Vec::new()),
        };
    }

    let elapsed = samples.len() as f32 / sample_rate as f32;
    if elapsed < MIN_RECORDING_SECS {
        println!("[audio] recording too short ({elapsed:.2}s), discarded");
        return Ok(Vec::new());
    }

    // With microphone access denied, macOS still delivers frames — all zeros —
    // instead of failing, so exact silence is how a denied permission shows up.
    if samples.iter().all(|s| *s == 0.0) {
        return Err(
            "El micrófono no devuelve señal. Revisá el permiso en Ajustes del Sistema > \
             Privacidad y seguridad > Micrófono."
                .to_string(),
        );
    }

    println!(
        "[audio] recording stopped ({:.1}s, {} samples)",
        elapsed,
        samples.len()
    );
    encode_wav(&samples, sample_rate)
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for s in samples {
            let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            writer.write_sample(v).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

pub async fn start(state: &AppState) -> Result<(), String> {
    // Set before Start goes out: a cancel that lands while the start is in
    // flight clears it, and the check below then undoes the start.
    state.recording_wanted.store(true, Ordering::SeqCst);
    let tx = sender(state)?;
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(RecorderCmd::Start(reply_tx))
        .map_err(|_| "El grabador de audio no responde".to_string())?;
    reply_rx
        .await
        .map_err(|_| "El grabador de audio no responde".to_string())??;
    if !state.recording_wanted.load(Ordering::SeqCst) {
        let _ = tx.send(RecorderCmd::Cancel);
        return Err("Grabación cancelada".to_string());
    }
    Ok(())
}

pub async fn stop(state: &AppState) -> Result<Vec<u8>, String> {
    state.recording_wanted.store(false, Ordering::SeqCst);
    let tx = sender(state)?;
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(RecorderCmd::Stop(reply_tx))
        .map_err(|_| "El grabador de audio no responde".to_string())?;
    reply_rx
        .await
        .map_err(|_| "El grabador de audio no responde".to_string())?
}

/// Fire-and-forget; also called from the popup's blur handler, so it must not
/// block or need the async runtime. Always sends Cancel (the recorder ignores
/// it when idle): the thread handles commands in order, so a Start sent
/// before this ends up cancelled.
pub fn cancel(state: &AppState) {
    state.recording_wanted.store(false, Ordering::SeqCst);
    if let Ok(slot) = state.recorder.lock() {
        if let Some(tx) = slot.as_ref() {
            let _ = tx.send(RecorderCmd::Cancel);
        }
    }
}
