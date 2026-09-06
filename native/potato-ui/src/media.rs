use crate::{App, Message};
use base64::{engine::general_purpose::STANDARD, Engine};
use iced::Task;
use serde_json::Value;
#[derive(Clone)]
pub enum Event {
    Pick,
    Picked(String, Result<Vec<Value>, String>),
    DropFile(std::path::PathBuf),
    FlushDrops,
    Clear,
    Remove(usize),
    SaveImage(String),
    Saved(Result<bool, String>),
}
impl App {
    pub fn media_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::DropFile(path) if !self.streaming => {
                self.pending_drops.push(path);
            }
            Event::Pick | Event::FlushDrops if !self.streaming && !self.busy => {
                self.busy = true;
                let destination = self.selected.clone().unwrap_or_default();
                let dropped = if matches!(event, Event::FlushDrops) {
                    Some(std::mem::take(&mut self.pending_drops))
                } else {
                    None
                };
                return Task::perform(
                    async move {
                        let paths = if let Some(paths) = dropped {
                            paths
                        } else {
                            rfd::AsyncFileDialog::new()
                                .pick_files()
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .map(|f| f.path().to_owned())
                                .collect()
                        };
                        tokio::task::spawn_blocking(move || {
                            paths
                                .iter()
                                .map(|path| {
                                    potato_core::attachments::upload_path(path)
                                        .map_err(|e| e.message)
                                })
                                .collect()
                        })
                        .await
                        .map_err(|_| "无法读取附件".to_owned())?
                    },
                    move |v| Message::Media(Event::Picked(destination.clone(), v)),
                );
            }
            Event::Picked(destination, result) => {
                self.busy = false;
                match result {
                    Ok(values) if destination == self.selected.clone().unwrap_or_default() => {
                        self.attachments.extend(values);
                        self.status = format!("已添加 {} 个附件", self.attachments.len());
                    }
                    Ok(values) => {
                        self.drafts
                            .entry(destination)
                            .or_default()
                            .attachments
                            .extend(values);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_attachments_stay_with_their_original_conversation() {
        let mut app = App {
            selected: Some("new".into()),
            ..App::default()
        };
        let value = serde_json::json!({"type":"file","file_name":"old.txt","file_url":"data:text/plain;base64,b2xk"});
        let _ = app.media_event(Event::Picked("old".into(), Ok(vec![value])));
        assert!(app.attachments.is_empty());
        assert_eq!(app.drafts["old"].attachments.len(), 1);
    }
    #[test]
    fn multiple_drop_events_are_collected_before_loading() {
        let mut app = App::default();
        let _ = app.media_event(Event::DropFile("a.txt".into()));
        let _ = app.media_event(Event::DropFile("b.txt".into()));
        assert_eq!(app.pending_drops.len(), 2);
        assert!(!app.busy);
    }
}
