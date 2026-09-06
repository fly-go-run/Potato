use crate::{App, Message};
use base64::{engine::general_purpose::STANDARD, Engine};
use iced::Task;
use serde_json::{json, Value};
#[derive(Clone)]
pub enum Event {
    Pick,
    Picked(Result<Option<Value>, String>),
    Clear,
    Remove(usize),
    SaveImage(String),
    Saved(Result<bool, String>),
}
impl App {
    pub fn media_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Pick if !self.streaming && !self.busy => {
                let Some(backend) = self.backend.clone() else {
                    return Task::none();
                };
                self.busy = true;
                return Task::perform(
                    async move {
                        let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
                            return Ok(None);
                        };
                        let name = file.file_name();
                        if std::fs::metadata(file.path())
                            .map_err(|_| "无法读取附件")?
                            .len()
                            > 20_000_000
                        {
                            return Err("附件超过 20 MB".into());
                        }
                        let bytes = file.read().await;
                        if bytes.len() > 20_000_000 {
                            return Err("附件超过 20 MB".into());
                        }
                        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                        let encoded = STANDARD.encode(bytes);
                        let value = if let Some(mime) = match ext.as_str() {
                            "png" => Some("image/png"),
                            "jpg" | "jpeg" => Some("image/jpeg"),
                            "webp" => Some("image/webp"),
                            _ => None,
                        } {
                            json!({"type":"image","image_url":format!("data:{mime};base64,{encoded}"),"file_name":name})
                        } else {
                            let v = backend
                                .request(
                                    "POST",
                                    "/api/console/upload",
                                    json!({"filename":name,"base64":encoded}),
                                )
                                .await?;
                            json!({"type":"file","file_url":v["url"],"file_name":v["file_name"]})
                        };
                        Ok(Some(value))
                    },
                    |v| Message::Media(Event::Picked(v)),
                );
            }
            Event::Picked(result) => {
                self.busy = false;
                match result {
                    Ok(Some(v)) => {
                        self.attachments.push(v);
                        self.status = format!("已添加 {} 个附件", self.attachments.len());
                    }
                    Ok(None) => {}
                    Err(e) => self.status = e,
                }
            }
            Event::Remove(index)
                if !self.streaming && !self.busy && index < self.attachments.len() =>
            {
                self.attachments.remove(index);
            }
            Event::Remove(_) => {}
            Event::Clear if !self.streaming && !self.busy => {
                self.attachments.clear();
                self.status.clear();
            }
            Event::Clear => {}
            Event::SaveImage(url) => {
                return Task::perform(
                    async move {
                        let (header, data) = url.split_once(",").ok_or("图片不是本地数据")?;
                        let filename = if header.contains("image/jpeg") {
                            "potato-image.jpg"
                        } else if header.contains("image/webp") {
                            "potato-image.webp"
                        } else {
                            "potato-image.png"
                        };
                        let bytes = STANDARD.decode(data).map_err(|_| "图片解码失败")?;
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .set_file_name(filename)
                            .save_file()
                            .await
                        {
                            file.write(&bytes).await.map_err(|_| "保存图片失败")?;
                            return Ok(true);
                        }
                        Ok(false)
                    },
                    |v| Message::Media(Event::Saved(v)),
                );
            }
            Event::Saved(v) => {
                self.status = v
                    .map(|saved| {
                        if saved {
                            "图片已保存".into()
                        } else {
                            "已取消保存".into()
                        }
                    })
                    .unwrap_or_else(|e| e)
            }
            _ => {}
        }
        Task::none()
    }
}
