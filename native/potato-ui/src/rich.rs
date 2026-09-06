//! Parsed presentation data is cached when messages change, never during view().
use crate::Message;
use iced::widget::{button, column, container, markdown, rich_text, row, scrollable, text, Column};
use iced::{Element, Fill, Theme};
use serde_json::Value;

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String,
    pub reasoning: bool,
    pub raw: Option<Value>,
    images: Vec<(String, iced::widget::image::Handle)>,
    pub body: String,
    blocks: Vec<Block>,
    tools: Vec<Tool>,
}
#[derive(Clone)]
struct Tool {
    call_id: Option<String>,
    is_output: bool,
    name: String,
    status: String,
    arguments: String,
    output: String,
}
impl From<(String, String)> for ChatMessage {
    fn from((role, body): (String, String)) -> Self {
        Self {
            role,
            reasoning: false,
            raw: None,
            images: vec![],
            blocks: parse_blocks(&body),
            body,
            tools: vec![],
        }
    }
}
#[derive(Clone)]
enum Block {
    Prose(Vec<markdown::Item>),
    Table {
        rows: Vec<Vec<Vec<markdown::Item>>>,
        alignments: Vec<pulldown_cmark::Alignment>,
    },
    Quote(Vec<Block>),
    Raw(String),
    Rule,
}

// Isolate unsupported blocks so a single table or image cannot flatten an answer.
fn parse_blocks(body: &str) -> Vec<Block> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let mut result = vec![];
    let mut cursor = 0;
    let mut events = Parser::new_ext(
        body,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    .peekable();
    while let Some((event, range)) = events.next() {
        let mut end = range.end;
        let special = match event {
            Event::Start(Tag::Table(alignments)) => {
                let mut rows = vec![];
                let mut cells = vec![];
                let mut cell_start = 0;
                for (event, r) in events.by_ref() {
                    end = r.end;
                    match event {
                        Event::Start(Tag::TableCell) => cell_start = r.start,
                        Event::End(TagEnd::TableCell) => {
                            cells.push(markdown::parse(body[cell_start..r.end].trim()).collect())
                        }
                        Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                            rows.push(std::mem::take(&mut cells))
                        }
                        Event::End(TagEnd::Table) => break,
                        _ => {}
                    }
                }
                Some(Block::Table { rows, alignments })
            }
            Event::Start(Tag::BlockQuote(_)) => {
                let mut depth = 1;
                for (event, r) in events.by_ref() {
                    end = r.end;
                    match event {
                        Event::Start(Tag::BlockQuote(_)) => depth += 1,
                        Event::End(TagEnd::BlockQuote) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let source = body[range.start..end]
                    .lines()
                    .map(|line| {
                        let line = line.trim_start();
                        line.strip_prefix('>')
                            .map(|s| s.strip_prefix(' ').unwrap_or(s))
                            .unwrap_or(line)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(Block::Quote(parse_blocks(&source)))
            }
            Event::Rule => Some(Block::Rule),
            Event::Html(_) | Event::InlineHtml(_) => Some(Block::Raw(body[range.clone()].into())),
            Event::Start(Tag::Image { .. }) => {
                for (event, r) in events.by_ref() {
                    end = r.end;
                    if matches!(event, Event::End(TagEnd::Image)) {
                        break;
                    }
                }
                Some(Block::Raw(body[range.start..end].into()))
            }
            _ => None,
        };
        if let Some(block) = special {
            if range.start > cursor {
                result.push(Block::Prose(
                    markdown::parse(&body[cursor..range.start]).collect(),
                ));
            }
            result.push(block);
            cursor = end;
        }
    }
    if cursor < body.len() {
        result.push(Block::Prose(markdown::parse(&body[cursor..]).collect()));
    }
    result
}

fn render_blocks<'a>(blocks: &'a [Block], style: markdown::Style) -> Element<'a, Message> {
    let mut content = Column::new().spacing(12).width(Fill);
    for block in blocks {
        content = content.push(match block {
            Block::Prose(items) => render_items(items, style),
            Block::Raw(source) => text(source).size(16).into(),
            Block::Rule => iced::widget::rule::horizontal(1).into(),
            Block::Quote(blocks) => {
                crate::hover::quote(container(render_blocks(blocks, style)).padding([0, 16]))
            }
            Block::Table { rows, alignments } => {
                let mut table = Column::new();
                for (index, cells) in rows.iter().enumerate() {
                    let mut line = iced::widget::Row::new();
                    for (column, cell) in cells.iter().enumerate() {
                        let align = match alignments.get(column) {
                            Some(pulldown_cmark::Alignment::Right) => {
                                iced::alignment::Horizontal::Right
                            }
                            Some(pulldown_cmark::Alignment::Center) => {
                                iced::alignment::Horizontal::Center
                            }
                            _ => iced::alignment::Horizontal::Left,
                        };
                        let mut cell_view = Column::new().width(Fill);
                        for item in cell {
                            if let markdown::Item::Paragraph(value)
                            | markdown::Item::Heading(_, value) = item
                            {
                                let element: Element<'a, markdown::Uri> =
                                    rich_text(value.spans(style))
                                        .size(14)
                                        .width(Fill)
                                        .align_x(align)
                                        .font(iced::Font {
                                            weight: if index == 0 {
                                                iced::font::Weight::Semibold
                                            } else {
                                                iced::font::Weight::Normal
                                            },
                                            ..iced::Font::DEFAULT
                                        })
                                        .into();
                                cell_view = cell_view
                                    .push(element.map(|url| Message::Link(url.to_string())));
                            }
                        }
                        line = line.push(
                            container(cell_view)
                                .width((680.0 / alignments.len().max(1) as f32).max(160.0))
                                .padding([8, 12])
                                .align_x(align),
                        );
                    }
                    table = table.push(line).push(iced::widget::rule::horizontal(1));
                }
                scrollable(table.width(
                    (680.0 / alignments.len().max(1) as f32).max(160.0)
                        * alignments.len().max(1) as f32,
                ))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .into()
            }
        });
    }
    content.into()
}

fn printable(value: &Value) -> String {
    if value.is_null() {
        String::new()
    } else if let Some(s) = value.as_str() {
        s.into()
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}
impl ChatMessage {
    pub fn from_value(value: &Value) -> Self {
        let (role, body) = crate::backend::plain_message(value);
        let mut result: Self = (role, body).into();
        result.reasoning = value["type"] == "reasoning";
        if result.role == "user" {
            result.raw = Some(value.clone());
        }
        if let Some(blocks) = value["content"].as_array() {
            let mut prose = Vec::new();
            for block in blocks {
                if block["type"] == "image" {
                    use base64::Engine;
                    let url = block["image_url"].as_str().unwrap_or("");
                    if url.starts_with("data:image/") {
                        if let Some((_, data)) = url.split_once(',') {
                            if let Ok(bytes) =
                                base64::engine::general_purpose::STANDARD.decode(data)
                            {
                                result.images.push((
                                    url.to_owned(),
                                    iced::widget::image::Handle::from_bytes(bytes),
                                ));
                                continue;
                            }
                        }
                    }
                }
                if block["type"] == "data"
                    && (block["data"].get("name").is_some()
                        || value["type"].as_str().is_some_and(|t| t.contains("call")))
                {
                    let data = &block["data"];
                    result.tools.push(Tool {
                        call_id: data["call_id"].as_str().map(str::to_owned),
                        is_output: value["type"] == "function_call_output",
                        name: data["name"].as_str().unwrap_or("工具").into(),
                        status: if value["type"] == "function_call" {
                            "in_progress"
                        } else {
                            value["status"].as_str().unwrap_or("unknown")
                        }
                        .into(),
                        arguments: printable(&data["arguments"]),
                        output: printable(&data["output"]),
                    });
                } else {
                    prose.push(
                        crate::backend::plain_message(&serde_json::json!({"content":[block]})).1,
                    );
                }
            }
            result.body = prose.join("\n\n");
            result.blocks = parse_blocks(&result.body);
        }
        result
    }
    pub fn images_view(&self) -> Element<'_, Message> {
        let mut content = Column::new().spacing(8);
        for (url, handle) in &self.images {
            content = content.push(
                iced::widget::image(handle.clone())
                    .width(Fill)
                    .height(420)
                    .content_fit(iced::ContentFit::Contain),
            );
            content = content.push(
                button("保存图片")
                    .on_press(Message::Media(crate::media::Event::SaveImage(url.clone()))),
            );
        }
        content.into()
    }
    pub fn view<'a>(
        &'a self,
        theme: &Theme,
        index: usize,
        expanded: &std::collections::HashSet<(usize, usize)>,
    ) -> Element<'a, Message> {
        let mut style = markdown::Style::from_palette(theme.palette());
        style.inline_code_color = theme.palette().text;
        style.inline_code_highlight.background = crate::ui::code_surface(theme).background.unwrap();
        style.inline_code_padding = iced::Padding::from([2., 6.]);
        let mut content = Column::new().spacing(12).width(Fill);
        if self.reasoning {
            let open = expanded.contains(&(index, usize::MAX));
            content = content.push(
                button(if open {
                    "▾ 思考过程"
                } else {
                    "▸ 思考过程"
                })
                .style(crate::ui::nav)
                .on_press(Message::ToggleTool(index, usize::MAX)),
            );
            if open {
                content = content.push(render_blocks(&self.blocks, style));
            }
        } else {
            content = content.push(render_blocks(&self.blocks, style));
        }
        content = content.push(self.images_view());
        for (tool_index, tool) in self.tools.iter().enumerate() {
            let open = expanded.contains(&(index, tool_index));
            let suffix = match tool.status.as_str() {
                "failed" => " · 失败",
                "cancelled" => " · 已取消",
                "in_progress" => " · 执行中",
                _ => "",
            };
            let mut record = column![button(
                row![
                    crate::ui::disclosure(open, theme),
                    text(format!("{}{suffix}", tool.name))
                        .size(13)
                        .color(crate::ui::muted(theme))
                ]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center)
            )
            .style(crate::ui::nav)
            .padding([6, 2])
            .on_press(Message::ToggleTool(index, tool_index))]
            .spacing(4);
            if open {
                let detail = format!(
                    "参数\n{}\n\n输出\n{}",
                    tool.arguments,
                    if tool.output.is_empty() {
                        "等待结果"
                    } else {
                        &tool.output
                    }
                );
                record = record.push(
                    container(column![
                        row![
                            iced::widget::Space::new().width(Fill),
                            crate::ui::copy_action(detail.clone(), theme)
                        ],
                        container(
                            scrollable(
                                text(detail)
                                    .font(iced::Font::MONOSPACE)
                                    .shaping(text::Shaping::Advanced)
                                    .size(12)
                                    .line_height(text::LineHeight::Absolute(24.0.into()))
                            )
                            .height(iced::Length::Shrink)
                        )
                        .max_height(220)
                    ])
                    .padding([8, 12])
                    .width(Fill)
                    .style(crate::ui::code_surface),
                );
            }
            content = content.push(record);
        }
        content.into()
    }
}
/// Core emits calls and results as separate messages. Join by call_id, never name.
pub fn merge_tools(messages: &mut Vec<ChatMessage>) {
    let mut calls = std::collections::HashMap::new();
    let mut result: Vec<ChatMessage> = Vec::new();
    for mut message in messages.drain(..) {
        let mut remaining = Vec::new();
        for tool in message.tools.drain(..) {
            if let Some(id) = &tool.call_id {
                if tool.is_output {
                    if let Some(&(m, t)) = calls.get(id) {
                        let target: &mut Tool = &mut result[m].tools[t];
                        target.output = tool.output;
                        target.status = tool.status;
                        continue;
                    }
                } else {
                    calls.insert(id.clone(), (result.len(), remaining.len()));
                }
            }
            remaining.push(tool);
        }
        message.tools = remaining;
        if !message.tools.is_empty() || !message.body.is_empty() || !message.images.is_empty() {
            result.push(message);
        }
    }
    *messages = result;
}

fn render_items<'a>(items: &'a [markdown::Item], style: markdown::Style) -> Element<'a, Message> {
    let mut content = Column::new().spacing(8).width(Fill);
    for item in items {
        match item {
            markdown::Item::List { start, items } => {
                for (i, item) in items.iter().enumerate() {
                    content = content.push(
                        row![
                            text(
                                start
                                    .map(|s| format!("{}.", s + i as u64))
                                    .unwrap_or("•".into())
                            )
                            .size(16),
                            render_items(item, style)
                        ]
                        .spacing(10),
                    );
                }
            }
            markdown::Item::CodeBlock(code) => {
                let mut spans = code.spans(style).to_vec();
                // Iced's bundled Markdown highlighter uses a dark syntax palette.
                // Adapt those colors to the original client's light code surface.
                if style.inline_code_color.r + style.inline_code_color.g + style.inline_code_color.b
                    < 1.5
                {
                    for span in &mut spans {
                        if let Some(color) = &mut span.color {
                            color.r *= 0.55;
                            color.g *= 0.55;
                            color.b *= 0.55;
                        }
                    }
                }
                let code_view: Element<'a, markdown::Uri> = rich_text(spans)
                    .width(iced::Length::Shrink)
                    .font(iced::Font::MONOSPACE)
                    .size(12)
                    .line_height(text::LineHeight::Absolute(24.0.into()))
                    .into();
                content = content.push(
                    container(
                        scrollable(code_view.map(|url| Message::Link(url.to_string()))).direction(
                            scrollable::Direction::Horizontal(scrollable::Scrollbar::default()),
                        ),
                    )
                    .padding([12, 16])
                    .width(Fill)
                    .style(crate::ui::code_surface),
                );
            }
            markdown::Item::Heading(_, value) | markdown::Item::Paragraph(value) => {
                let mut spans = value.spans(style).to_vec();
                for span in &mut spans {
                    if let Some(font) = &mut span.font {
                        if font.family != iced::font::Family::Monospace {
                            font.family =
                                iced::font::Family::Name(if cfg!(target_os = "windows") {
                                    "Microsoft YaHei"
                                } else {
                                    "PingFang SC"
                                });
                            // These CJK system families have no italic face. Avoid missing-glyph runs.
                            if span.text.chars().any(|c| c as u32 >= 0x2e80) {
                                font.style = iced::font::Style::Normal;
                            }
                        }
                    }
                }
                let size = match item {
                    markdown::Item::Heading(level, _) => match level {
                        markdown::HeadingLevel::H1 => 20,
                        markdown::HeadingLevel::H2 => 18,
                        _ => 16,
                    },
                    _ => 16,
                };
                let heading = matches!(item, markdown::Item::Heading(..));
                let font = iced::Font {
                    family: iced::font::Family::Name(if cfg!(target_os = "windows") {
                        "Microsoft YaHei"
                    } else {
                        "PingFang SC"
                    }),
                    weight: if heading {
                        iced::font::Weight::Semibold
                    } else {
                        iced::font::Weight::Normal
                    },
                    ..Default::default()
                };
                let element: Element<'a, markdown::Uri> = rich_text(spans)
                    .font(font)
                    .size(size)
                    .line_height(text::LineHeight::Absolute(28.0.into()))
                    .into();
                content = content.push(element.map(|url| Message::Link(url.to_string())));
            }
        }
    }
    content.into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_chat_view_accepts_nested_rich_content_in_vertical_scroller() {
        let app = crate::App {
            selected: Some("test".into()),
            messages: vec![(
                "assistant".into(),
                "> 引用\n> > 嵌套\n\n| A | B |\n|---|---|\n| x | y |\n\n```rust\nfn main() {}\n```"
                    .into(),
            )
                .into()],
            ..crate::App::default()
        };
        let _ = app.view();
    }
    #[test]
    fn tables_keep_surrounding_prose_inline_markup_and_alignment() {
        let blocks = parse_blocks("**前文**\n\n| A | B |\n|:---|---:|\n| **中文** | `x` |\n\n后文");
        assert!(matches!(&blocks[0], Block::Prose(_)));
        let Block::Table { rows, alignments } = &blocks[1] else {
            panic!("missing table");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].len(), 2);
        assert_eq!(alignments[1], pulldown_cmark::Alignment::Right);
        let markdown::Item::Paragraph(value) = &rows[1][0][0] else {
            panic!("missing cell");
        };
        let spans = value.spans(markdown::Style::from_palette(Theme::Light.palette()));
        assert_eq!(spans[0].text, "中文");
        assert!(spans[0].font.is_some());
        assert!(matches!(blocks.last(), Some(Block::Prose(_))));
    }
    #[test]
    fn quotes_and_rules_are_rendered_and_code_is_not_reparsed_as_table() {
        let blocks =
            parse_blocks("> 引用\n>\n> > 嵌套\n\n---\n\n```text\n| a | b |\n|---|---|\n```");
        assert!(matches!(blocks[0], Block::Quote(_)));
        assert!(blocks.iter().any(|b| matches!(b, Block::Rule)));
        assert!(!blocks.iter().any(|b| matches!(b, Block::Table { .. })));
    }
    #[test]
    fn every_stream_prefix_of_complex_unicode_markdown_is_safe() {
        let body = "前文\n\n| 中文 | B |\n|---|---|\n| **内容** | `x` |\n\n> 引用\n> > 子引用\n\n![图片](https://example.com/a.png)\n\n```rust\nfn main() {}\n```";
        for end in (0..=body.len()).filter(|i| body.is_char_boundary(*i)) {
            let blocks = parse_blocks(&body[..end]);
            let _ = render_blocks(
                &blocks,
                markdown::Style::from_palette(Theme::Light.palette()),
            );
        }
    }
    #[test]
    fn unsupported_markdown_is_preserved_and_stream_prefixes_parse() {
        for source in [
            "| A | B |\n|---|---|\n| x | y |",
            "![image](https://example.com/a.png)",
            "<div>keep</div>",
        ] {
            let message: ChatMessage = ("assistant".into(), source.into()).into();
            assert!(!message.blocks.is_empty());
            assert_eq!(message.body, source);
        }
        let source = "## 标题\n\n- **粗体**\n- `code`\n\n```rust\nfn main() {}\n```";
        for end in (0..=source.len()).filter(|i| source.is_char_boundary(*i)) {
            let _: ChatMessage = ("assistant".into(), source[..end].into()).into();
        }
    }
    #[test]
    fn structured_tools_keep_arguments_and_output() {
        let message = ChatMessage::from_value(
            &serde_json::json!({"role":"assistant","type":"tool_call","status":"failed","content":[{"type":"data","data":{"name":"shell","arguments":{"command":"pwd"},"output":"denied"}},{"type":"text","text":"**说明**"}]}),
        );
        assert_eq!(message.tools.len(), 1);
        assert!(message.tools[0].arguments.contains("pwd"));
        assert_eq!(message.tools[0].output, "denied");
        assert_eq!(message.tools[0].status, "failed");
        assert_eq!(message.body, "**说明**");
    }
}
