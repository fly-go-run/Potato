//! Document extraction is performed on upload, never during desktop startup.
//! Uploaded document text is inert data; Office macros and external links are
//! never executed or fetched. Originals on the user's computer are untouched.
use crate::{required, Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use quick_xml::{events::Event, Reader};
use serde_json::{json, Value};
use std::io::{Cursor, Read};

fn xml_text(xml: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    let mut result = String::new();
    loop {
        match reader
            .read_event()
            .map_err(|_| Error::new(400, "Invalid Office XML"))?
        {
            Event::Start(tag) if tag.local_name().as_ref() == "t" => {
                let text = reader
                    .read_text(tag.name())
                    .map_err(|_| Error::new(400, "Invalid Office text"))?;
                let text = text.into_inner();
                result.push_str(
                    &quick_xml::escape::unescape(&text)
                        .map_err(|_| Error::new(400, "Invalid Office XML entity"))?,
                );
            }
            Event::End(tag) if matches!(tag.local_name().as_ref(), "p" | "tr") => result.push('\n'),
            Event::End(tag) if tag.local_name().as_ref() == "tc" => result.push('\t'),
            Event::Empty(tag) if matches!(tag.local_name().as_ref(), "br" | "cr") => {
                result.push('\n')
            }
            Event::Empty(tag) if tag.local_name().as_ref() == "tab" => result.push('\t'),
            Event::DocType(_) => {
                return Err(Error::new(
                    400,
                    "Office XML document types are not supported",
                ))
            }
            Event::Eof => break,
            _ => {}
        }
        if result.len() > 1_000_000 {
            return Err(Error::new(413, "Extracted text exceeds 1 MB"));
        }
    }
    Ok(result)
}

fn office(bytes: &[u8], extension: &str) -> Result<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| Error::new(400, "Invalid Office archive"))?;
    if zip.len() > 5000 {
        return Err(Error::new(413, "Office archive contains too many entries"));
    }
    let mut entries = Vec::new();
    let mut expanded = 0u64;
    for index in 0..zip.len() {
        let file = zip
            .by_index(index)
            .map_err(|_| Error::new(400, "Cannot read Office archive"))?;
        if file.enclosed_name().is_none() {
            return Err(Error::new(400, "Invalid Office archive path"));
        }
        expanded = expanded.saturating_add(file.size());
        if expanded > 100_000_000 {
            return Err(Error::new(413, "Expanded Office archive exceeds 100 MB"));
        }
        let name = file.name();
        if (extension == "docx"
            && (name == "word/document.xml"
                || name == "word/footnotes.xml"
                || name == "word/endnotes.xml"))
            || (extension == "pptx"
                && name.starts_with("ppt/slides/slide")
                && name.ends_with(".xml")
                && !name.contains("/_rels/"))
        {
            entries.push(name.to_owned());
        }
    }
    entries.sort_by_key(|name| {
        if extension == "pptx" {
            name.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<u32>()
                .unwrap_or(u32::MAX)
        } else if name == "word/document.xml" {
            0
        } else {
            1
        }
    });
    if entries.is_empty() {
        return Err(Error::new(
            400,
            "Office document contains no readable text parts",
        ));
    }
    let mut result = String::new();
    for (index, name) in entries.iter().enumerate() {
        let file = zip
            .by_name(name)
            .map_err(|_| Error::new(400, "Missing Office text part"))?;
        let mut xml = String::new();
        file.take(10_000_001).read_to_string(&mut xml)?;
        if xml.len() > 10_000_000 {
            return Err(Error::new(413, "Office text part is too large"));
        }
        if extension == "pptx" {
            result.push_str(&format!("\nSlide {}\n", index + 1));
        }
        result.push_str(&xml_text(&xml)?);
        if result.len() > 1_000_000 {
            return Err(Error::new(413, "Extracted text exceeds 1 MB"));
        }
    }
    Ok(result)
}

pub(crate) fn upload(body: Value) -> Result<Value> {
    let filename = required(&body, "filename")?;
    if filename.len() > 255 || filename.contains(['\0', '/', '\\']) {
        return Err(Error::new(400, "Invalid attachment filename"));
    }
    let encoded = required(&body, "base64")?;
    if encoded.len() > 28_000_000 {
        return Err(Error::new(413, "Attachment exceeds 20 MB"));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| Error::new(400, "Invalid attachment encoding"))?;
    if bytes.len() > 20_000_000 {
        return Err(Error::new(413, "Attachment exceeds 20 MB"));
    }
    let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let (text, extracted) = if bytes.starts_with(b"%PDF-") {
        let text = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes))
            .map_err(|_| Error::new(400, "PDF parser could not read this document"))?
            .map_err(|_| Error::new(400, "PDF is invalid, encrypted or unsupported"))?;
        if text.trim().is_empty() {
            return Err(Error::new(
                415,
                "This PDF has no extractable text; scanned documents need OCR",
            ));
        }
        (text, true)
    } else if matches!(extension.as_str(), "xlsx" | "xls" | "xlsb" | "xlsm" | "ods") {
        use calamine::Reader as _;
        let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(bytes))
            .map_err(|_| Error::new(400, "Spreadsheet is invalid, encrypted or unsupported"))?;
        let names = workbook.sheet_names();
        if names.len() > 100 {
            return Err(Error::new(413, "Spreadsheet has more than 100 sheets"));
        }
        let mut text = String::new();
        for name in names {
            let range = workbook
                .worksheet_range(&name)
                .map_err(|_| Error::new(400, "Cannot read spreadsheet worksheet"))?;
            if range.get_size().0.saturating_mul(range.get_size().1) > 1_000_000 {
                return Err(Error::new(413, "Worksheet exceeds one million cells"));
            }
            text.push_str(&format!("\nWorksheet: {name}\n"));
            for row in range.rows() {
                for (index, cell) in row.iter().enumerate() {
                    if index > 0 {
                        text.push('\t');
                    }
                    text.push_str(&cell.to_string().replace(['\t', '\n'], " "));
                }
                text.push('\n');
                if text.len() > 1_000_000 {
                    return Err(Error::new(413, "Extracted spreadsheet text exceeds 1 MB"));
                }
            }
        }
        (text, true)
    } else if matches!(extension.as_str(), "docx" | "pptx") {
        (office(&bytes, &extension)?, true)
    } else {
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::new(415, "Unsupported binary document format"))?;
        if text.contains('\0') {
            return Err(Error::new(415, "Binary document is not text"));
        }
        (text, false)
    };
    if text.len() > 1_000_000 {
        return Err(Error::new(413, "Extracted text exceeds 1 MB"));
    }
    Ok(
        json!({"url":format!("data:text/plain;base64,{}",STANDARD.encode(text.as_bytes())),"file_name":if extracted{format!("{filename}.txt")}else{filename.to_owned()},"original_file_name":filename,"size":text.len(),"extracted":extracted}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn office_text_preserves_unicode_entities_and_slide_order() {
        let doc=archive(&[("word/document.xml","<w:document><w:p><w:r><w:t>你好 &amp; family</w:t></w:r><w:tab/><w:r><w:t>42</w:t></w:r></w:p></w:document>")]);
        let result =
            upload(json!({"filename":"family.docx","base64":STANDARD.encode(doc)})).unwrap();
        let text = String::from_utf8(
            STANDARD
                .decode(result["url"].as_str().unwrap().split_once(',').unwrap().1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(text, "你好 & family\t42\n");
        assert_eq!(result["file_name"], "family.docx.txt");
        let slides = archive(&[
            ("ppt/slides/slide10.xml", "<a:p><a:t>ten</a:t></a:p>"),
            ("ppt/slides/slide2.xml", "<a:p><a:t>two</a:t></a:p>"),
        ]);
        let text = office(&slides, "pptx").unwrap();
        assert!(text.find("two").unwrap() < text.find("ten").unwrap());
    }
    #[test]
    fn external_entities_and_archive_traversal_are_rejected() {
        assert!(xml_text("<!DOCTYPE x SYSTEM 'file:///etc/passwd'><x/>").is_err());
        assert!(office(&archive(&[("../word/document.xml", "<x/>")]), "docx").is_err());
        assert!(
            upload(json!({"filename":"bad.pdf","base64":STANDARD.encode(b"%PDF-broken")})).is_err()
        );
    }

    #[test]
    fn pdf_text_is_extracted_without_an_external_converter() {
        let stream = "BT /F1 12 Tf 20 200 Td (Hello PDF family) Tj ET";
        let objects=["<< /Type /Catalog /Pages 2 0 R >>".to_owned(),"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),format!("<< /Length {} >>\nstream\n{stream}\nendstream",stream.len())];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in &offsets[1..] {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        let result = upload(json!({"filename":"hello.pdf","base64":STANDARD.encode(pdf)})).unwrap();
        let text = String::from_utf8(
            STANDARD
                .decode(result["url"].as_str().unwrap().split_once(',').unwrap().1)
                .unwrap(),
        )
        .unwrap();
        assert!(text.contains("Hello PDF family"));
        assert_eq!(result["file_name"], "hello.pdf.txt");
    }

    #[test]
    fn spreadsheet_reads_named_sheets_and_cached_values_without_running_formulas() {
        let bytes = archive(&[
            (
                "_rels/.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "[Content_Types].xml",
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Family" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>你好</t></is></c><c r="B1"><f>1+1</f><v>2</v></c></row></sheetData></worksheet>"#,
            ),
        ]);
        let result =
            upload(json!({"filename":"family.xlsx","base64":STANDARD.encode(bytes)})).unwrap();
        let text = String::from_utf8(
            STANDARD
                .decode(result["url"].as_str().unwrap().split_once(',').unwrap().1)
                .unwrap(),
        )
        .unwrap();
        assert!(text.contains("Worksheet: Family"));
        assert!(text.contains("你好\t2"));
    }
}
