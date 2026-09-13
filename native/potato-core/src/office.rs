//! Native, bounded Office generation. Libraries only receive in-memory content:
//! no user-selected templates, scripts, network resources or file paths.
use crate::{required, Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{io::Cursor, path::Path};
mod charts;
mod spreadsheet;
pub(crate) mod template;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Deck {
    title: String,
    slides: Vec<Slide>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Slide {
    title: String,
    bullets: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    chart: Option<charts::ChartSpec>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    title: String,
    paragraphs: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Workbook {
    sheets: Vec<Sheet>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sheet {
    name: String,
    rows: Vec<Vec<Value>>,
    #[serde(default)]
    header: bool,
    #[serde(default)]
    column_widths: Vec<f64>,
    #[serde(default)]
    number_format: Option<String>,
}

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::new(400, format!("Office generation failed: {error}"))
}
fn bounded(count: usize, max: usize, label: &str) -> Result<()> {
    if count == 0 || count > max {
        return Err(Error::new(
            400,
            format!("{label} must contain 1..={max} items"),
        ));
    }
    Ok(())
}
fn text(value: &str) -> Result<()> {
    if value.len() > 32_000
        || value.chars().any(|c| {
            !matches!(c, '\t' | '\n' | '\r')
                && (c < '\u{20}' || matches!(c, '\u{fffe}' | '\u{ffff}'))
        })
    {
        return Err(Error::new(
            400,
            "Office text is too long or contains invalid XML characters",
        ));
    }
    Ok(())
}

/// Conservative content-density hints, explicitly distinct from font shaping
/// and visual rendering. Stable chart rectangles are fixed in charts.rs.
pub(crate) fn layout_warnings(args: &Value) -> Vec<String> {
    let units = |s: &str| {
        s.chars()
            .map(|c| if c.is_ascii() { 1 } else { 2 })
            .sum::<usize>()
    };
    let mut warnings = Vec::new();
    if let Some(slides) = args["document"]["slides"].as_array() {
        for (i, slide) in slides.iter().enumerate() {
            if units(slide["title"].as_str().unwrap_or("")) > 52 {
                warnings.push(format!(
                    "Slide {}: long title may wrap or overflow; inspect in PowerPoint",
                    i + 1
                ));
            }
            if let Some(bullets) = slide["bullets"].as_array() {
                let lines: usize = bullets
                    .iter()
                    .map(|b| {
                        b.as_str()
                            .unwrap_or("")
                            .split('\n')
                            .map(|s| units(s).div_ceil(64).max(1))
                            .sum::<usize>()
                    })
                    .sum();
                if lines > 9 {
                    warnings.push(format!(
                        "Slide {}: dense body text may overflow; split into more slides",
                        i + 1
                    ));
                }
            }
            if let Some(chart) = slide.get("chart") {
                let long_labels = chart["categories"].as_array().is_some_and(|items| {
                    items.len() > 12 || items.iter().any(|v| units(v.as_str().unwrap_or("")) > 20)
                });
                if long_labels {
                    warnings.push(format!(
                        "Slide {}: chart labels may crowd; shorten labels or use fewer categories",
                        i + 1
                    ));
                }
            }
        }
    }
    warnings
}

pub(crate) fn generate(target: &Path, args: &Value) -> Result<Vec<u8>> {
    let format = required(args, "format")?;
    if target
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some(format)
    {
        return Err(Error::new(400, "Office format must match output extension"));
    }
    let data = &args["document"];
    if serde_json::to_vec(data)?.len() > 1_000_000 {
        return Err(Error::new(413, "Office input exceeds 1 MB"));
    }
    let bytes = match format {
        "pptx" => {
            let deck: Deck = serde_json::from_value(data.clone())?;
            text(&deck.title)?;
            bounded(deck.slides.len(), 100, "Slides")?;
            let mut slides = Vec::new();
            let mut chart_packages = Vec::new();
            for slide in deck.slides {
                text(&slide.title)?;
                if slide.bullets.len() > 30 {
                    return Err(invalid("At most 30 bullets per slide"));
                }
                let mut output = ppt_rs::generator::SlideContent::new(&slide.title);
                if let Some(chart) = slide.chart {
                    if !slide.bullets.is_empty() {
                        return Err(invalid(
                            "Chart slides require empty bullets; use a separate text slide",
                        ));
                    }
                    chart.validate()?;
                    output = output
                        .with_layout(ppt_rs::generator::SlideLayout::TitleOnly)
                        .add_chart(chart.placeholder());
                    chart_packages.push(chart.package()?);
                }
                for bullet in slide.bullets {
                    text(&bullet)?;
                    output = output.add_bullet(&bullet);
                }
                if let Some(notes) = slide.notes {
                    text(&notes)?;
                    output.notes = Some(notes);
                }
                slides.push(output);
            }
            let bytes = ppt_rs::generator::create_pptx_with_content(&deck.title, slides)
                .map_err(invalid)?;
            let bytes = charts::replace_chart_packages(bytes, chart_packages)?;
            if !ppt_rs::validate_package_bytes(&bytes).is_valid() {
                return Err(invalid("PPTX package validation failed"));
            }
            bytes
        }
        "docx" => {
            use docx_rs::{Docx, Paragraph, Run};
            let document: Document = serde_json::from_value(data.clone())?;
            text(&document.title)?;
            bounded(document.paragraphs.len(), 2000, "Paragraphs")?;
            let mut doc = Docx::new().add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(&document.title).bold().size(36)),
            );
            for paragraph in document.paragraphs {
                text(&paragraph)?;
                doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(paragraph)));
            }
            let mut output = Cursor::new(Vec::new());
            doc.pack(&mut output).map_err(invalid)?;
            output.into_inner()
        }
        "xlsx" => {
            let input: Workbook = serde_json::from_value(data.clone())?;
            spreadsheet::generate(input)?
        }
        _ => return Err(invalid("Unsupported Office format")),
    };
    if bytes.len() > 20_000_000 {
        return Err(Error::new(413, "Office output exceeds 20 MB"));
    }
    // Check required OPC parts and the main document entry before publishing.
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).map_err(invalid)?;
    let main = match format {
        "pptx" => "ppt/presentation.xml",
        "docx" => "word/document.xml",
        _ => "xl/workbook.xml",
    };
    for entry in ["[Content_Types].xml", "_rels/.rels", main] {
        archive.by_name(entry).map_err(invalid)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde_json::json;
    use std::io::Read;

    fn part(bytes: &[u8], name: &str) -> String {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut text = String::new();
        zip.by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    #[test]
    fn office_files_round_trip_chinese_and_xml_entities() {
        for (format, document) in [
            (
                "docx",
                json!({"title":"研发 <计划> & 预算", "paragraphs":["中文正文：第一阶段", "第二阶段"]}),
            ),
            (
                "pptx",
                json!({"title":"研发 <计划> & 预算", "slides":[{"title":"研发 <计划> & 预算","bullets":["中文正文：第一阶段"],"notes":"讲者备注 <检查> & 保留"},{"title":"第二阶段","bullets":[]}]}),
            ),
        ] {
            let filename = format!("test.{format}");
            let bytes = generate(
                Path::new(&filename),
                &json!({"format":format,"document":document}),
            )
            .unwrap();
            // Use the independent native attachment extractor, not the writer.
            let extracted = crate::attachments::upload(
                json!({"filename":filename,"base64":STANDARD.encode(&bytes)}),
            )
            .unwrap();
            let encoded = extracted["url"]
                .as_str()
                .unwrap()
                .split_once(',')
                .unwrap()
                .1;
            let text = String::from_utf8(STANDARD.decode(encoded).unwrap()).unwrap();
            assert!(text.contains("研发 <计划> & 预算"), "{text}");
            assert!(text.contains("中文正文：第一阶段"), "{text}");
            assert!(text.contains("第二阶段"), "{text}");
            if format == "pptx" {
                let notes = part(&bytes, "ppt/notesSlides/notesSlide1.xml");
                assert!(notes.contains("讲者备注"));
                assert!(notes.contains("&lt;检查&gt;"));
            }
        }
    }

    #[test]
    fn spreadsheet_preserves_types_and_never_interprets_strings_as_formulas() {
        use calamine::Reader;
        let bytes = generate(Path::new("test.xlsx"), &json!({"format":"xlsx","document":{"sheets":[{"name":"预算","rows":[["项目","金额","完成"],["研发",12.5,true],["=1+1",null,false]]}]}})).unwrap();
        let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(&bytes)).unwrap();
        let sheet = workbook.worksheet_range("预算").unwrap();
        assert_eq!(sheet.get_value((1, 1)), Some(&calamine::Data::Float(12.5)));
        assert_eq!(sheet.get_value((1, 2)), Some(&calamine::Data::Bool(true)));
        assert_eq!(
            sheet.get_value((2, 0)),
            Some(&calamine::Data::String("=1+1".into()))
        );
        assert!(!part(&bytes, "xl/worksheets/sheet1.xml").contains("<f>"));
    }

    #[test]
    fn invalid_office_input_is_rejected() {
        for args in [
            json!({"format":"pptx","document":{"title":"x","slides":[]}}),
            json!({"format":"pptx","document":{"title":"\u{0}","slides":[{"title":"x","bullets":[]}]}}),
            json!({"format":"pptx","document":{"title":"x","slides":[{"title":"x","bullets":[],"template_path":"/tmp/x"}]}}),
        ] {
            assert!(generate(Path::new("test.pptx"), &args).is_err());
        }
        assert!(generate(
            Path::new("test.docx"),
            &json!({"format":"xlsx","document":{}})
        )
        .is_err());
        assert!(generate(Path::new("test.xlsx"), &json!({"format":"xlsx","document":{"sheets":[{"name":"x","rows":[[{"formula":"=UNSUPPORTED(1)"}]]}]}})).is_err());
    }

    #[test]
    fn ppt_template_replaces_slide_and_notes_without_changing_layout_parts() {
        let source = generate(Path::new("template.pptx"), &json!({"format":"pptx","document":{"title":"模板","slides":[{"title":"{{title}}","bullets":["客户：{{name}}"],"notes":"{{name}} 的备注"}]}})).unwrap();
        let result = template::fill(
            source.clone(),
            "pptx",
            &json!({"{{title}}":"新标题","{{name}}":"新客户"}),
        )
        .unwrap();
        assert!(part(&result, "ppt/slides/slide1.xml").contains("新标题"));
        assert!(part(&result, "ppt/notesSlides/notesSlide1.xml").contains("新客户 的备注"));
        assert_eq!(
            part(&source, "ppt/presentation.xml"),
            part(&result, "ppt/presentation.xml")
        );
        assert_eq!(
            part(&source, "ppt/slideMasters/slideMaster1.xml"),
            part(&result, "ppt/slideMasters/slideMaster1.xml")
        );
    }

    #[test]
    fn layout_hints_flag_dense_content_without_claiming_rendering() {
        let args = json!({"document":{"slides":[{"title":"长标题".repeat(30),"bullets":vec!["一行";12]}]}});
        assert_eq!(layout_warnings(&args).len(), 2);
    }

    #[test]
    fn calculated_formulas_have_real_caches_and_reject_errors() {
        use calamine::Reader;
        let args = json!({"format":"xlsx","document":{"sheets":[{"name":"预算","header":true,"column_widths":[24,18],"number_format":"0.00","rows":[["项目","金额"],["A",10],["B",20],["总计",{"formula":"=SUM(B2:B3)"}],["均值",{"formula":"=B4/2"}],["四舍五入",{"formula":"=ROUND(AVERAGE(B2:B3)/7,2)"}]]}]}});
        let bytes = generate(Path::new("budget.xlsx"), &args).unwrap();
        let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(&bytes)).unwrap();
        let range = workbook.worksheet_range("预算").unwrap();
        assert_eq!(range.get_value((3, 1)), Some(&calamine::Data::Float(30.0)));
        assert_eq!(range.get_value((4, 1)), Some(&calamine::Data::Float(15.0)));
        assert_eq!(range.get_value((5, 1)), Some(&calamine::Data::Float(2.14)));
        assert!(part(&bytes, "xl/worksheets/sheet1.xml").contains("<f>SUM(B2:B3)</f>"));
        assert!(part(&bytes, "xl/worksheets/sheet1.xml").contains("state=\"frozen\""));
        for formula in [
            "=1/0",
            "=A1",
            "=SUM(A:A)",
            "=SUM(A1:A999999)",
            "=WEBSERVICE(1)",
            "=SUM(",
        ] {
            assert!(generate(Path::new("bad.xlsx"),&json!({"format":"xlsx","document":{"sheets":[{"name":"x","rows":[[{"formula":formula}]]}]}})).is_err(),"{formula}");
        }
    }

    #[test]
    fn editable_ppt_chart_matches_real_embedded_workbook() {
        use calamine::Reader;
        let bytes = generate(Path::new("charts.pptx"),&json!({"format":"pptx","document":{"title":"图表","slides":[{"title":"销售数据","bullets":[],"chart":{"kind":"column","title":"实际 & 计划","categories":["第一季度","第二季度"],"series":[{"name":"实际","values":[12,24]},{"name":"计划","values":[15,30]}]}}]}})).unwrap();
        let xml = part(&bytes, "ppt/charts/chart1.xml");
        assert!(xml.contains("Sheet1!$C$2:$C$3"), "{xml}");
        assert!(xml.contains("<c:v>30</c:v>"));
        assert!(xml.contains("<c:externalData r:id=\"rId1\""));
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut embedded = Vec::new();
        archive
            .by_name("ppt/embeddings/Microsoft_Excel_Sheet1.xlsx")
            .unwrap()
            .read_to_end(&mut embedded)
            .unwrap();
        let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(embedded)).unwrap();
        let range = workbook.worksheet_range("Sheet1").unwrap();
        assert_eq!(range.get_value((2, 2)), Some(&calamine::Data::Float(30.0)));
        assert_eq!(
            range.get_value((0, 2)),
            Some(&calamine::Data::String("计划".into()))
        );
        let slide = part(&bytes, "ppt/slides/slide1.xml");
        assert!(slide.contains("4937760"));
    }

    #[test]
    fn template_fill_keeps_all_unmodified_parts_and_fails_missing_keys() {
        let source = generate(Path::new("template.docx"),&json!({"format":"docx","document":{"title":"{{title}}","paragraphs":["客户：{{name}}","客户：{{name}}"]}})).unwrap();
        let output = template::fill(
            source.clone(),
            "docx",
            &json!({"{{title}}":"报告","{{name}}":"中文 & <客户>"}),
        )
        .unwrap();
        assert!(part(&output, "word/document.xml").contains("中文 &amp; &lt;客户&gt;"));
        let mut original = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
        let mut result = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
        assert_eq!(original.len(), result.len());
        for i in 0..original.len() {
            let mut file = original.by_index(i).unwrap();
            if file.name() == "word/document.xml" {
                continue;
            }
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            result
                .by_name(file.name())
                .unwrap()
                .read_to_end(&mut actual)
                .unwrap();
            file.read_to_end(&mut expected).unwrap();
            assert_eq!(expected, actual);
        }
        assert!(template::fill(source, "docx", &json!({"{{missing}}":"x"})).is_err());
    }
}
