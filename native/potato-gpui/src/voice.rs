//! Speech capture reuses the proven PCM conversion and the existing Rust speech service.
use crate::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
static CAPTURE_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct CaptureGuard;
impl CaptureGuard {
    fn acquire() -> Result<Self, String> {
        CAPTURE_BUSY
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .map(|_| Self)
            .map_err(|_| "上一次麦克风操作尚未退出，请检查系统输入设备后重试".into())
    }
}
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        CAPTURE_BUSY.store(false, std::sync::atomic::Ordering::Release);
    }
}

#[derive(Default)]
pub struct Voice {
    pub active: bool,
    stop: Option<mpsc::Sender<()>>,
}
impl Potato {
    pub fn toggle_voice(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.voice.active {
            if let Some(stop) = self.voice.stop.take() {
                let _ = stop.send(());
            }
            return;
        }
        if self.streaming {
            return;
        }
        let (stop, rx) = mpsc::channel();
        self.voice.stop = Some(stop.clone());
        self.voice.active = true;
        self.notice = "正在打开麦克风…".into();
        let prefix = self.composer.read(cx).value().to_string();
        let session = self.session.clone();
        let core = self.backend.core.clone();
        let (events, mut frames) = futures::channel::mpsc::unbounded();
        self.backend.executor.spawn(async move {
            let id = uuid::Uuid::new_v4().to_string();
            let (prepared_tx, prepared_rx) = tokio::sync::oneshot::channel();
            let (start_tx, start_rx) = mpsc::channel();
            let task_core = core.clone();
            let task_id = id.clone();
            let task_events = events.clone();
            let handle = tokio::runtime::Handle::current();
            std::thread::spawn(move || {
                let result = capture(task_core.clone(), &task_id, rx, task_events.clone(), prepared_tx, start_rx);
                if let Err(e) = &result {
                    let _ = task_events.unbounded_send(json!({"type":"error","message":e}));
                }
                handle.spawn(async move {
                    let _ = task_core.voice_end(&task_id, result.is_err()).await;
                });
            });
            // CoreAudio may block while opening an unavailable input device. Do
            // not start a billed ASR stream before the microphone is prepared.
            match tokio::time::timeout(Duration::from_secs(12), prepared_rx).await {
                Ok(Ok(())) => {},
                Ok(Err(_)) => return, // capture already emitted the device error
                Err(_) => {
                    let _ = stop.send(());
                    let _ = events.unbounded_send(json!({"type":"error","message":"麦克风初始化超时，请检查系统输入设备和麦克风权限后重试"}));
                    return;
                }
            }
            let tx = events.clone();
            if let Err(e) = core.voice_start(id.clone(), Arc::new(move |v| {
                tx.unbounded_send(v).map_err(|_| potato_core::Error::new(499, "Voice window closed"))
            })).await {
                let _ = stop.send(());
                let _ = events.unbounded_send(json!({"type":"error","message":e.message}));
                return;
            }
            if start_tx.send(()).is_err() {
                let _ = core.voice_end(&id, true).await;
                return;
            }
            tokio::time::sleep(Duration::from_secs(195)).await;
            let _ = stop.send(());
            let _ = core.voice_end(&id, true).await;
            let _ = events.unbounded_send(json!({"type":"timeout"}));
        });
        cx.spawn_in(w, async move |this, cx| {
            while let Some(frame) = frames.next().await {
                let done = matches!(frame["type"].as_str(), Some("error" | "final" | "timeout"));
                let alive = this
                    .update_in(cx, |s, w, cx| {
                        if frame["type"] == "ready" {
                            s.notice = "正在录音，点击麦克风结束".into();
                        }
                        if let Some(text) = frame["text"].as_str() {
                            let value = format!(
                                "{}{}{}",
                                prefix,
                                if prefix.is_empty() { "" } else { " " },
                                text
                            );
                            if s.session == session {
                                s.composer.update(cx, |v, cx| v.set_value(value, w, cx));
                            } else {
                                s.drafts.entry(session.clone()).or_default().0 = value;
                            }
                        }
                        if done {
                            s.voice.active = false;
                            if let Some(stop) = s.voice.stop.take() {
                                let _ = stop.send(());
                            }
                            s.notice = if frame["type"] == "error" {
                                string(&frame, "message")
                            } else if frame["type"] == "timeout" {
                                "语音识别超时".into()
                            } else {
                                "语音已填入，可编辑后发送".into()
                            };
                        }
                        cx.notify();
                    })
                    .is_ok();
                if done || !alive {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
}
struct Pcm {
    rate: u32,
    channels: usize,
    phase: u64,
    sum: f32,
    count: u32,
    bytes: Vec<u8>,
}
impl Pcm {
    fn samples<T: cpal::Sample>(&mut self, data: &[T])
    where
        f32: cpal::FromSample<T>,
    {
        for frame in data.chunks_exact(self.channels) {
            self.sum +=
                frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / self.channels as f32;
            self.count += 1;
            self.phase += 16000;
            while self.phase >= self.rate as u64 {
                let sample =
                    ((self.sum / self.count as f32).clamp(-1., 1.) * 32767.).round() as i16;
                self.bytes.extend(sample.to_le_bytes());
                self.phase -= self.rate as u64;
            }
            if self.phase < 16000 {
                self.sum = 0.;
                self.count = 0;
            }
        }
    }
}
fn input<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    core: Arc<potato_core::Runtime>,
    id: String,
    errors: mpsc::Sender<String>,
    tail: Arc<std::sync::Mutex<Vec<u8>>>,
) -> Result<cpal::Stream, String>
where
    f32: cpal::FromSample<T>,
{
    let mut pcm = Pcm {
        rate: config.sample_rate.0,
        channels: config.channels as usize,
        phase: 0,
        sum: 0.,
        count: 0,
        bytes: Vec::new(),
    };
    let error_tx = errors.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                pcm.samples(data);
                if pcm.bytes.len() >= 3200 {
                    let bytes = std::mem::take(&mut pcm.bytes);
                    for part in bytes.chunks(64000) {
                        if let Err(e) = core.voice_audio(&id, part.to_vec()) {
                            let _ = errors.send(e.message);
                            break;
                        }
                    }
                }
                if let Ok(mut pending) = tail.lock() {
                    *pending = pcm.bytes.clone();
                }
            },
            move |_| {
                let _ = error_tx.send("麦克风采集中断".into());
            },
            None,
        )
        .map_err(|_| "无法打开麦克风，请检查系统权限和输入设备".into())
}
fn capture(
    core: Arc<potato_core::Runtime>,
    id: &str,
    stop: mpsc::Receiver<()>,
    events: futures::channel::mpsc::UnboundedSender<serde_json::Value>,
    prepared: tokio::sync::oneshot::Sender<()>,
    start_recording: mpsc::Receiver<()>,
) -> Result<(), String> {
    let _guard = CaptureGuard::acquire()?;
    if !matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
        return Ok(());
    }
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("没有可用麦克风")?;
    let config = device
        .default_input_config()
        .map_err(|_| "无法读取麦克风配置")?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let (errors, rx) = mpsc::channel();
    let tail = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stream = match format {
        cpal::SampleFormat::F32 => input::<f32>(
            &device,
            &config,
            core.clone(),
            id.into(),
            errors,
            tail.clone(),
        ),
        cpal::SampleFormat::I16 => input::<i16>(
            &device,
            &config,
            core.clone(),
            id.into(),
            errors,
            tail.clone(),
        ),
        cpal::SampleFormat::U16 => input::<u16>(
            &device,
            &config,
            core.clone(),
            id.into(),
            errors,
            tail.clone(),
        ),
        _ => Err("麦克风采样格式暂不支持".into()),
    }?;
    if prepared.send(()).is_err()
        || start_recording
            .recv_timeout(Duration::from_secs(15))
            .is_err()
    {
        return Ok(());
    }
    if !matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
        return Ok(());
    }
    stream.play().map_err(|_| "无法开始录音")?;
    let _ = events.unbounded_send(serde_json::json!({"type":"ready"}));
    let start = std::time::Instant::now();
    loop {
        if let Ok(error) = rx.try_recv() {
            return Err(error);
        }
        match stop.recv_timeout(Duration::from_millis(50)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if start.elapsed() > Duration::from_secs(180) {
            break;
        }
    }
    drop(stream);
    let bytes = std::mem::take(&mut *tail.lock().map_err(|_| "录音缓冲区不可用")?);
    if !bytes.is_empty() {
        core.voice_audio(id, bytes).map_err(|e| e.message)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn stalled_capture_cannot_spawn_another_device_initialization() {
        let guard = CaptureGuard::acquire().unwrap();
        assert!(CaptureGuard::acquire().is_err());
        drop(guard);
        assert!(CaptureGuard::acquire().is_ok());
    }
    #[::core::prelude::v1::test]
    fn stereo_48k_becomes_mono_16k_across_chunks() {
        let mut pcm = Pcm {
            rate: 48000,
            channels: 2,
            phase: 0,
            sum: 0.,
            count: 0,
            bytes: vec![],
        };
        for _ in 0..100 {
            pcm.samples(&vec![0.5f32; 960]);
        }
        assert_eq!(pcm.bytes.len(), 32000);
        assert!(
            pcm.bytes
                .chunks_exact(2)
                .all(|s| i16::from_le_bytes([s[0], s[1]]) == 16384)
        );
    }
}
