//! Microphone opens only after a user click; PCM stays in memory.
use crate::{App, Message};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::{SinkExt, StreamExt};
use iced::{widget::text_editor, Task};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};
#[derive(Clone)]
pub enum Event {
    Toggle,
    Ready(mpsc::Sender<()>),
    Text(String),
    Finished(Result<(), String>),
}
#[derive(Default)]
pub struct Voice {
    pub active: bool,
    stop: Option<mpsc::Sender<()>>,
    prefix: String,
}
impl App {
    pub fn voice_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Toggle if self.voice.active => {
                if let Some(stop) = self.voice.stop.take() {
                    let _ = stop.send(());
                }
                self.status = "正在完成语音识别…".into();
            }
            Event::Toggle if !self.streaming && !self.busy => {
                let Some(backend) = self.backend.clone() else {
                    return Task::none();
                };
                let (stop, rx) = mpsc::channel();
                self.voice.stop = Some(stop.clone());
                self.voice.active = true;
                self.voice.prefix = self.draft.text();
                self.status = "正在连接语音服务…".into();
                return Task::run(
                    iced::stream::channel(16, async move |mut output| {
                        let id = uuid::Uuid::new_v4().to_string();
                        let core = backend.core.clone();
                        let result:Result<(),String>=async {
                        let (events,mut frames)=iced::futures::channel::mpsc::unbounded();
                        let tx=events.clone();
                        core.voice_start(id.clone(),Arc::new(move|v|tx.unbounded_send(v).map_err(|_|potato_core::Error::new(499,"Voice window closed")))).await.map_err(|e|e.message)?;
                        let thread_core=core.clone();let thread_id=id.clone();
                        let handle=tokio::runtime::Handle::current();
                        std::thread::spawn(move||{
                            let result=capture(thread_core.clone(),&thread_id,rx,events.clone());
                            if let Err(error)=&result {let _=events.unbounded_send(serde_json::json!({"type":"error","message":error}));}
                            handle.spawn(async move{let _=thread_core.voice_end(&thread_id,result.is_err()).await;});
                        });
                        let timeout=tokio::time::sleep(Duration::from_secs(195));tokio::pin!(timeout);
                        loop {
                            let frame=tokio::select!{_= &mut timeout=>return Err("语音识别超时".into()),v=frames.next()=>v.ok_or("语音连接已结束")?};
                            if frame["type"]=="ready" {output.send(Event::Ready(stop.clone())).await.map_err(|_|"窗口已关闭")?;}
                            if frame["type"]=="error" {return Err(frame["message"].as_str().unwrap_or("语音识别失败").into())}
                            if let Some(text)=frame["text"].as_str(){output.send(Event::Text(text.into())).await.map_err(|_|"窗口已关闭")?;}
                            if frame["type"]=="final" {let _=stop.send(());break;}
                        }
                        Ok(())
                    }.await;
                        let _ = core.voice_end(&id, true).await;
                        let _ = output.send(Event::Finished(result)).await;
                    }),
                    Message::Voice,
                );
            }
            Event::Ready(stop) => {
                if self.voice.active {
                    self.voice.stop = Some(stop);
                    self.status = "正在录音，点击麦克风结束".into();
                } else {
                    let _ = stop.send(());
                }
            }
            Event::Text(text) => {
                self.draft = text_editor::Content::with_text(&format!(
                    "{}{}{}",
                    self.voice.prefix,
                    if self.voice.prefix.is_empty() {
                        ""
                    } else {
                        " "
                    },
                    text
                ));
            }
            Event::Finished(result) => {
                self.voice.active = false;
                self.voice.stop.take();
                self.status = result
                    .map(|_| "语音已填入，可编辑后发送".into())
                    .unwrap_or_else(|e| e);
            }
            _ => {}
        }
        Task::none()
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
    events: iced::futures::channel::mpsc::UnboundedSender<serde_json::Value>,
) -> Result<(), String> {
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
    #[test]
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
        assert!(pcm
            .bytes
            .chunks_exact(2)
            .all(|s| i16::from_le_bytes([s[0], s[1]]) == 16384));
    }
}
