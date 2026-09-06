//! Port of agents/utils/doubao_stream_asr.py: 16 kHz mono PCM, gzip framing.
use crate::{lock, string, Emit, Error, Result, Runtime};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

pub(crate) enum Input {
    Audio(Vec<u8>),
    Stop,
    Cancel,
}
const ENDPOINT: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";

fn encode(kind: u8, flags: u8, json: bool, data: &[u8]) -> Result<Vec<u8>> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(data)?;
    let data = gzip.finish()?;
    let mut bytes = vec![0x11, (kind << 4) | flags, if json { 0x11 } else { 0x01 }, 0];
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend(data);
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Option<Value>> {
    if bytes.len() < 8 || bytes[0] >> 4 != 1 {
        return Err(Error::new(502, "Invalid speech frame"));
    }
    let kind = bytes[1] >> 4;
    let flags = bytes[1] & 15;
    let mut offset = ((bytes[0] & 15) as usize) * 4;
    if offset < 4 {
        return Err(Error::new(502, "Invalid speech header"));
    }
    if flags & 1 != 0 {
        offset += 4;
    }
    if kind == 15 {
        let code = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| Error::new(502, "Truncated speech error frame"))?;
        let code = u32::from_be_bytes(code.try_into().unwrap());
        return Err(Error::new(
            502,
            format!("豆包语音服务返回错误（代码 {code}），请检查录音设备、网络和语音配置"),
        ));
    }
    if kind != 9 {
        return Ok(None);
    }
    let size = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| Error::new(502, "Truncated speech frame"))?;
    let size = u32::from_be_bytes(size.try_into().unwrap()) as usize;
    let payload = bytes
        .get(offset + 4..offset + 4 + size)
        .ok_or_else(|| Error::new(502, "Truncated speech payload"))?;
    let decoded = if bytes[2] & 15 == 1 {
        let mut decoded = Vec::new();
        GzDecoder::new(payload)
            .take(1_000_001)
            .read_to_end(&mut decoded)?;
        decoded
    } else {
        payload.to_vec()
    };
    if decoded.len() > 1_000_000 {
        return Err(Error::new(502, "Speech frame too large"));
    }
    let value: Value = serde_json::from_slice(&decoded)?;
    let result = if value["result"].is_array() {
        &value["result"][0]
    } else {
        &value["result"]
    };
    let text = string(result, "text");
    // "definite" marks a finalized utterance, not the end of the whole stream.
    // Keep listening until the protocol LAST flag so long speech isn't cut off.
    if text.is_empty() && flags & 2 == 0 {
        return Ok(None);
    }
    Ok(Some(
        json!({"type":if flags&2!=0 {"final"}else{"partial"},"text":text}),
    ))
}

impl Runtime {
    pub async fn voice_start(self: &Arc<Self>, id: String, emit: Emit) -> Result<()> {
        self.voice_connect(id, emit, ENDPOINT).await
    }

    async fn voice_connect(self: &Arc<Self>, id: String, emit: Emit, endpoint: &str) -> Result<()> {
        let settings = self.db()?.get("doubao", json!({}))?;
        if settings["enabled"] != true {
            return Err(Error::new(400, "Doubao speech is disabled"));
        }
        let key = self.db()?.unseal(string(&settings, "api_key"))?;
        if key.is_empty() {
            return Err(Error::new(
                400,
                "Configure Doubao speech credentials in Settings first",
            ));
        }
        let mut request = endpoint
            .into_client_request()
            .map_err(|_| Error::new(400, "Invalid speech endpoint"))?;
        let mut headers = vec![
            (
                "X-Api-Resource-Id",
                settings["resource_id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("volc.seedasr.sauc.duration")
                    .to_owned(),
            ),
            ("X-Api-Connect-Id", id.clone()),
        ];
        if string(&settings, "app_id").is_empty() {
            headers.push(("X-Api-Key", key));
        } else {
            headers.push(("X-Api-App-Key", string(&settings, "app_id").into()));
            headers.push(("X-Api-Access-Key", key));
        }
        for (name, value) in headers {
            request.headers_mut().insert(
                name,
                value
                    .parse()
                    .map_err(|_| Error::new(400, "Invalid speech credential header"))?,
            );
        }
        let (tx, mut rx) = mpsc::channel(32);
        {
            let mut voices = lock(&self.voices)?;
            if !voices.is_empty() {
                return Err(Error::new(409, "A voice recording is already active"));
            }
            voices.insert(id.clone(), tx);
        }
        let connection =
            tokio::time::timeout(Duration::from_secs(12), connect_async(request)).await;
        let mut socket = match connection {
            Ok(Ok((socket, _))) => socket,
            _ => {
                lock(&self.voices)?.remove(&id);
                return Err(Error::new(
                    502,
                    "Could not connect to Doubao speech service",
                ));
            }
        };
        let config = json!({"user":{"uid":"potato-composer"},"audio":{"format":"pcm","codec":"raw","rate":16000,"bits":16,"channel":1},
            "request":{"model_name":"bigmodel","enable_itn":true,"enable_punc":true,"show_utterances":true,"result_type":"full"}});
        if socket
            .send(Message::Binary(
                encode(1, 0, true, config.to_string().as_bytes())?.into(),
            ))
            .await
            .is_err()
        {
            lock(&self.voices)?.remove(&id);
            return Err(Error::new(502, "Speech handshake failed"));
        }
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            let deadline = tokio::time::sleep(Duration::from_secs(130));
            tokio::pin!(deadline);
            let mut stopped = false;
            let mut total = 0usize;
            let result:Result<()>=async {
                loop {
                    tokio::select! {
                        _=&mut deadline=>return Err(Error::new(504,"Speech recognition timed out")),
                        input=rx.recv()=>match input {
                            Some(Input::Audio(bytes)) if !stopped=> {
                                total+=bytes.len();if total>4_160_000 {return Err(Error::new(413,"Voice recording exceeds 130 seconds"));}
                                socket.send(Message::Binary(encode(2,0,false,&bytes)?.into())).await.map_err(|_|Error::new(502,"Speech connection closed"))?;
                            }
                            Some(Input::Stop) if !stopped=>{
                                stopped=true;
                                deadline.as_mut().reset(tokio::time::Instant::now()+Duration::from_secs(8));
                                socket.send(Message::Binary(encode(2,2,false,&[])?.into())).await.map_err(|_|Error::new(502,"Speech connection closed"))?;
                            }
                            Some(Input::Cancel)|None=>return Ok(()),
                            _=>{}
                        },
                        message=socket.next()=>match message {
                            Some(Ok(Message::Binary(bytes)))=>if let Some(frame)=decode(&bytes)? {
                                let final_frame=frame["type"]=="final";emit(frame)?;
                                if final_frame{return Ok(());}
                            },
                            Some(Ok(Message::Ping(bytes)))=>{socket.send(Message::Pong(bytes)).await.map_err(|_|Error::new(502,"Speech connection closed"))?;}
                            Some(Ok(Message::Close(_)))|None=>return Err(Error::new(502,"Speech stream closed before its final result")),
                            Some(Err(_))=>return Err(Error::new(502,"Speech stream failed")),
                            _=>{}
                        }
                    }
                }
            }.await;
            if let Err(error) = result {
                let _ = emit(
                    json!({"type":"error","code":"TRANSCRIPTION_FAILED","message":error.message}),
                );
            }
            if let Ok(mut voices) = lock(&runtime.voices) {
                voices.remove(&id);
            }
            let _ = tokio::time::timeout(Duration::from_secs(1), socket.close(None)).await;
        });
        Ok(())
    }

    pub fn voice_audio(&self, id: &str, bytes: Vec<u8>) -> Result<()> {
        if bytes.len() > 65_536 || !bytes.len().is_multiple_of(2) {
            return Err(Error::new(413, "Invalid PCM chunk size"));
        }
        let voices = lock(&self.voices)?;
        voices
            .get(id)
            .ok_or_else(|| Error::new(404, "Voice session ended"))?
            .try_send(Input::Audio(bytes))
            .map_err(|_| Error::new(429, "Speech service is not keeping up"))
    }
    pub async fn voice_end(&self, id: &str, cancel: bool) -> Result<()> {
        let tx = lock(&self.voices)?.get(id).cloned();
        if let Some(tx) = tx {
            tx.send(if cancel { Input::Cancel } else { Input::Stop })
                .await
                .map_err(|_| Error::new(409, "Voice session ended"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_error_preserves_code_without_exposing_payload() {
        let mut frame = vec![0x11, 0xf0, 0x10, 0];
        frame.extend_from_slice(&45000081u32.to_be_bytes());
        frame.extend_from_slice(&6u32.to_be_bytes());
        frame.extend_from_slice(b"secret");
        let error = decode(&frame).unwrap_err();
        assert!(error.message.contains("45000081"));
        assert!(!error.message.contains("secret"));
    }
    #[test]
    fn decodes_definite_utterance_without_prematurely_ending_stream() {
        let value = json!({"result":{"text":"你好","utterances":[{"definite":true}]}});
        assert_eq!(
            decode(&encode(9, 0, true, value.to_string().as_bytes()).unwrap())
                .unwrap()
                .unwrap()["type"],
            "partial"
        );
        assert_eq!(
            decode(&encode(9, 2, true, value.to_string().as_bytes()).unwrap())
                .unwrap()
                .unwrap()["type"],
            "final"
        );
        assert!(decode(&[0x11, 0x90, 0x11, 0, 0, 0, 0, 99]).is_err());
    }

    #[tokio::test]
    async fn streams_pcm_to_doubao_and_delivers_partial_then_final() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        runtime.request("PUT","/api/native/doubao-settings",json!({"api_key":"test-speech-key","app_id":"","resource_id":"volc.seedasr.sauc.duration","enabled":true})).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                tcp,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.headers()["X-Api-Key"], "test-speech-key");
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let initial = socket.next().await.unwrap().unwrap().into_data();
            assert_eq!(initial[1], 0x10);
            let audio = socket.next().await.unwrap().unwrap().into_data();
            assert_eq!(audio[1], 0x20);
            socket
                .send(Message::Binary(
                    encode(9, 0, true, br#"{"result":{"text":"hello"}}"#)
                        .unwrap()
                        .into(),
                ))
                .await
                .unwrap();
            let last = socket.next().await.unwrap().unwrap().into_data();
            assert_eq!(last[1], 0x22);
            socket
                .send(Message::Binary(
                    encode(9, 2, true, br#"{"result":{"text":"hello world"}}"#)
                        .unwrap()
                        .into(),
                ))
                .await
                .unwrap();
        });
        let (tx, mut rx) = mpsc::unbounded_channel();
        runtime
            .voice_connect(
                "voice-test".into(),
                Arc::new(move |v| {
                    tx.send(v).unwrap();
                    Ok(())
                }),
                &endpoint,
            )
            .await
            .unwrap();
        runtime.voice_audio("voice-test", vec![0, 0, 1, 0]).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap()["type"],
            "partial"
        );
        runtime.voice_end("voice-test", false).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap()["text"],
            "hello world"
        );
        server.await.unwrap();
    }
}
