//! Fill literal placeholders without rebuilding unrelated OOXML parts.
//! Paragraph text is joined for matching, but replacement is applied to runs:
//! inserted text inherits the first run's formatting; remaining runs survive.
use super::*;
use quick_xml::{events::Event, Reader};
use std::{collections::BTreeMap, io::Read};

struct Node {
    xml: std::ops::Range<usize>,
    text: String,
    opening: String,
}

fn replace_paragraph(
    nodes: &mut Vec<Node>,
    values: &BTreeMap<String, String>,
    counts: &mut BTreeMap<String, usize>,
    edits: &mut Vec<(std::ops::Range<usize>, String)>,
    budget: &mut usize,
) -> Result<()> {
    let joined: String = nodes.iter().map(|n| n.text.as_str()).collect();
    let mut matches = Vec::new();
    for (key, value) in values {
        for (start, _) in joined.match_indices(key) {
            matches.push((start, start + key.len(), key, value));
        }
    }
    matches.sort_by_key(|m| m.0);
    if matches.windows(2).any(|m| m[0].1 > m[1].0) {
        return Err(invalid("Overlapping template placeholders"));
    }
    for (_, _, _, value) in &matches {
        *budget = budget.saturating_add(value.len());
        if *budget > 10_000_000 {
            return Err(invalid("Expanded template text exceeds 10 MB"));
        }
    }
    let mut starts = Vec::new();
    let mut offset = 0;
    for node in nodes.iter() {
        starts.push(offset);
        offset += node.text.len();
    }
    let mut texts: Vec<String> = nodes.iter().map(|n| n.text.clone()).collect();
    for (start, end, key, value) in matches.into_iter().rev() {
        *counts.entry(key.clone()).or_default() += 1;
        let mut first = true;
        for (i, node) in nodes.iter().enumerate() {
            let left = start.max(starts[i]);
            let right = end.min(starts[i] + node.text.len());
            if left < right {
                texts[i].replace_range(
                    left - starts[i]..right - starts[i],
                    if first { value } else { "" },
                );
                first = false;
            }
        }
    }
    for (node, value) in nodes.drain(..).zip(texts) {
        if node.text != value {
            edits.push((
                node.xml,
                format!("{}{}", node.opening, quick_xml::escape::escape(&value)),
            ));
        }
    }
    Ok(())
}

fn fill_xml(
    xml: &str,
    values: &BTreeMap<String, String>,
    counts: &mut BTreeMap<String, usize>,
) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    let mut nodes = Vec::new();
    let mut edits = Vec::new();
    let mut budget = xml.len();
    loop {
        let event_start = reader.buffer_position() as usize;
        match reader.read_event().map_err(invalid)? {
            Event::Start(tag) if matches!(tag.name().as_ref(), "w:t" | "a:t") => {
                let start = reader.buffer_position() as usize;
                let mut opening = tag.clone().into_owned();
                opening.clear_attributes();
                for attribute in tag.attributes() {
                    let attribute = attribute.map_err(invalid)?;
                    if attribute.key.as_ref() != "xml:space" {
                        opening.push_attribute(attribute);
                    }
                }
                opening.push_attribute(("xml:space", "preserve"));
                let mut writer = quick_xml::Writer::new(Vec::new());
                writer.write_event(Event::Start(opening)).map_err(invalid)?;
                let raw = reader.read_text(tag.name()).map_err(invalid)?;
                let raw = raw.into_inner();
                let value = quick_xml::escape::unescape(&raw)
                    .map_err(invalid)?
                    .into_owned();
                nodes.push(Node {
                    xml: event_start..start + raw.len(),
                    text: value,
                    opening: String::from_utf8(writer.into_inner()).map_err(invalid)?,
                });
            }
            Event::End(tag) if tag.local_name().as_ref() == "p" => {
                replace_paragraph(&mut nodes, values, counts, &mut edits, &mut budget)?
            }
            Event::Empty(tag) if matches!(tag.local_name().as_ref(), "br" | "tab") => {
                replace_paragraph(&mut nodes, values, counts, &mut edits, &mut budget)?
            }
            Event::DocType(_) => return Err(invalid("Office DTDs are not supported")),
            Event::Eof => break,
            _ => {}
        }
    }
    replace_paragraph(&mut nodes, values, counts, &mut edits, &mut budget)?;
    let mut output = xml.to_owned();
    for (range, value) in edits.into_iter().rev() {
        output.replace_range(range, &value);
    }
    Ok(output)
}

pub(crate) fn fill(bytes: Vec<u8>, format: &str, values: &Value) -> Result<Vec<u8>> {
    if !matches!(format, "docx" | "pptx") {
        return Err(invalid("Templates support DOCX and PPTX"));
    }
    if serde_json::to_vec(values)?.len() > 1_000_000 {
        return Err(invalid("Template replacements exceed 1 MB"));
    }
    let values: BTreeMap<String, String> = serde_json::from_value(values.clone())?;
    bounded(values.len(), 100, "Replacements")?;
    for (key, value) in &values {
        if !key.starts_with("{{") || !key.ends_with("}}") || key.len() > 100 || key.len() < 5 {
            return Err(invalid("Use literal {{name}} placeholder keys"));
        }
        text(key)?;
        text(value)?;
        if value.contains(['\n', '\r', '\t']) {
            return Err(invalid("Template replacements must be single-line text"));
        }
    }
    if bytes.len() > 20_000_000 {
        return Err(invalid("Template exceeds 20 MB"));
    }
    let mut input = zip::ZipArchive::new(Cursor::new(&bytes)).map_err(invalid)?;
    if input.len() > 5000 {
        return Err(invalid("Too many template entries"));
    }
    let main = if format == "docx" {
        "word/document.xml"
    } else {
        "ppt/presentation.xml"
    };
    for name in [main, "[Content_Types].xml", "_rels/.rels"] {
        input.by_name(name).map_err(invalid)?;
    }
    let mut expanded = 0u64;
    let mut seen = std::collections::HashSet::new();
    let mut changed = BTreeMap::new();
    let mut counts = BTreeMap::new();
    let mut changed_bytes = 0usize;
    for i in 0..input.len() {
        let file = input.by_index(i).map_err(invalid)?;
        let name = file.name().to_owned();
        expanded = expanded.saturating_add(file.size());
        if expanded > 100_000_000
            || file.enclosed_name().is_none()
            || name.contains(['\\', ':'])
            || Path::new(&name)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !seen.insert(name.clone())
        {
            return Err(invalid("Unsafe, duplicate or oversized template entries"));
        }
        if name.contains("vbaProject") || name.starts_with("_xmlsignatures/") {
            return Err(invalid(
                "Signed and macro-enabled templates are not supported",
            ));
        }
        let selected = if format == "docx" {
            name == main
                || name.starts_with("word/header")
                || name.starts_with("word/footer")
                || matches!(name.as_str(), "word/footnotes.xml" | "word/endnotes.xml")
        } else {
            name.starts_with("ppt/slides/slide") || name.starts_with("ppt/notesSlides/notesSlide")
        };
        if selected && name.ends_with(".xml") && !name.contains("/_rels/") {
            let mut xml = String::new();
            file.take(10_000_001).read_to_string(&mut xml)?;
            if xml.len() > 10_000_000 {
                return Err(invalid("Template XML exceeds 10 MB"));
            }
            let output = fill_xml(&xml, &values, &mut counts)?;
            changed_bytes = changed_bytes.saturating_add(output.len());
            if changed_bytes > 100_000_000 {
                return Err(invalid("Expanded template output exceeds 100 MB"));
            }
            if output != xml {
                changed.insert(name, output.into_bytes());
            }
        }
    }
    for key in values.keys() {
        if counts.get(key).copied().unwrap_or(0) == 0 {
            return Err(invalid(format!("Template placeholder not found: {key}")));
        }
    }
    drop(input);
    let result = super::charts::rewrite(bytes, changed)?;
    if result.len() > 20_000_000 {
        return Err(invalid("Template output exceeds 20 MB"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_runs_preserve_formatting_and_escape_replacement() {
        let xml = "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>客户：{{na</w:t></w:r><w:r><w:t>me}}，结束</w:t></w:r></w:p>";
        let values = BTreeMap::from([("{{name}}".into(), "研发 & 客户".into())]);
        let output = fill_xml(xml, &values, &mut BTreeMap::new()).unwrap();
        assert!(output.contains("<w:b/>"));
        assert!(output.contains("客户：研发 &amp; 客户</w:t>"));
        assert!(output.contains("，结束</w:t>"));
        assert!(output.contains("xml:space=\"preserve\""));
    }
}
