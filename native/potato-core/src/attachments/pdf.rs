//! Page-aware PDF extraction; PDF text is normalized only in radical blocks.
use crate::{Error, Result};
use unicode_normalization::UnicodeNormalization;
#[path = "radicals.rs"]
mod radicals;

fn normalize(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        if ('\u{2f00}'..='\u{2fd5}').contains(&c) {
            result.extend(std::iter::once(c).nfkc());
        } else {
            result.push(radicals::unified(c));
        }
    }
    result
}

pub(super) fn extract(bytes: &[u8]) -> Result<String> {
    std::panic::catch_unwind(|| extract_pages(bytes))
        .map_err(|_| Error::new(400, "PDF parser could not read this document"))?
}

fn extract_pages(bytes: &[u8]) -> Result<String> {
    let invalid = |_| Error::new(400, "PDF is invalid, encrypted or unsupported");
    let mut document = lopdf::Document::load_mem(bytes).map_err(invalid)?;
    if document.is_encrypted() {
        document.decrypt("").map_err(invalid)?;
    }
    let mut result = String::new();
    let mut has_text = false;
    // The upstream by_pages convenience API stops on ANY page error and returns
    // Ok(partial_text). Enumerate actual pages and propagate errors instead.
    for page_number in document.get_pages().keys() {
        let mut page = String::new();
        pdf_extract::output_doc_page(
            &document,
            &mut pdf_extract::PlainTextOutput::new(&mut page),
            *page_number,
        )
        .map_err(|_| Error::new(400, format!("Cannot extract PDF page {page_number}")))?;
        has_text |= !page.trim().is_empty();
        result.push_str(&format!("--- Page {page_number} ---\n"));
        result.push_str(&normalize(&page));
        result.push('\n');
        if result.len() > 1_000_000 {
            return Err(Error::new(413, "Extracted text exceeds 1 MB"));
        }
    }
    if !has_text {
        return Err(Error::new(
            415,
            "This PDF has no extractable text; scanned documents need OCR",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_radicals_without_changing_other_compatibility_characters() {
        assert_eq!(normalize("第⼀章 ⽣⽂⽬⾦⼆⻚⽤"), "第一章 生文目金二页用");
        let unchanged = "正常中文 Ａ①² ﬁ ㎏ ㍿ ⺀ ⺚\nEnglish";
        assert_eq!(normalize(unchanged), unchanged);
        assert_eq!(normalize("⺇⻳"), "\u{20628}龟");
    }

    #[test]
    fn real_pingfang_pdf_has_searchable_chinese_and_page_numbers() {
        let text = extract(include_bytes!(
            "../../tests/fixtures/attachments/pingfang.pdf"
        ))
        .unwrap();
        assert!(text.contains("第一章 概述"), "{text}");
        assert!(
            text.contains("本季度完成原生文件生成，验证中文内容。"),
            "{text}"
        );
        assert!(text.contains("第二页：附录内容，用于检查分页。"), "{text}");
        assert_eq!(text.matches("--- Page ").count(), 2);
        assert!(text.find("--- Page 2 ---").unwrap() < text.find("第二页").unwrap());
        assert!(!text.chars().any(|c| ('\u{2f00}'..='\u{2fd5}').contains(&c)));
    }

    fn sample(pages: &[&str]) -> lopdf::Document {
        use lopdf::{dictionary, Object, Stream};
        let mut doc = lopdf::Document::with_version("1.4");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(
            dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
        );
        let mut kids = Vec::new();
        for text in pages {
            let content = doc.add_object(Stream::new(dictionary! {}, text.as_bytes().to_vec()));
            kids.push(Object::Reference(doc.add_object(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
                "Resources" => dictionary! {"Font" => dictionary! {"F1" => font}}, "Contents" => content
            })));
        }
        doc.objects.insert(
            pages_id,
            dictionary! {"Type" => "Pages", "Count" => pages.len() as i64, "Kids" => kids}.into(),
        );
        let root = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages_id});
        doc.trailer.set("Root", root);
        doc
    }

    fn bytes(mut doc: lopdf::Document) -> Vec<u8> {
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn blank_pages_keep_positions_and_all_blank_still_needs_ocr() {
        let text = extract(&bytes(sample(&["", "BT /F1 12 Tf (Middle) Tj ET", ""]))).unwrap();
        assert_eq!(text.matches("--- Page ").count(), 3);
        assert!(text.find("--- Page 2 ---").unwrap() < text.find("Middle").unwrap());
        assert!(text.find("Middle").unwrap() < text.find("--- Page 3 ---").unwrap());
        assert_eq!(extract(&bytes(sample(&["", ""]))).unwrap_err().status, 415);
    }

    #[test]
    fn later_page_error_does_not_publish_partial_text() {
        let mut doc = sample(&["BT /F1 12 Tf (First) Tj ET", ""]);
        let second = doc.get_pages()[&2];
        doc.get_object_mut(second)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"MediaBox");
        assert!(extract(&bytes(doc)).is_err());
    }
}
