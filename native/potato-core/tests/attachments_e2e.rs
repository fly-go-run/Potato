//! Native upload → extraction → real HTTP model request → SSE → persisted history.
//! Providers are local fixtures; no credentials or external programs are needed.
use base64::{engine::general_purpose::STANDARD, Engine};
use potato_core::Runtime;
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};

async fn fixture(responses: Vec<(String, String)>) -> (String, mpsc::UnboundedReceiver<String>) {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for (content_type, body) in responses {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let size = socket.read(&mut chunk).await.unwrap();
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..size]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length: ")
                                .and_then(|n| n.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            tx.send(String::from_utf8_lossy(&request).to_string()).ok();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            // Exercise arbitrary TCP chunking, including inside CJK characters.
            for chunk in body.as_bytes().chunks(7) {
                if socket.write_all(chunk).await.is_err() {
                    break;
                }
            }
        }
    });
    (url, rx)
}

fn sse(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn answer(text: &str) -> String {
    sse(json!({"choices":[{"delta":{"content":text},"finish_reason":null}]}))
        + &sse(json!({"choices":[{"delta":{},"finish_reason":"stop"}]}))
        + "data: [DONE]\n\n"
}
async fn configure(runtime: &Runtime, url: &str, protocol: &str) {
    runtime
        .request(
            "PUT",
            "/api/models/deepseek/config",
            json!({"api_key":"secret-test-key","base_url":url,"chat_model":protocol}),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":"deepseek","model":"test-model"}),
        )
        .await
        .unwrap();
}
async fn finish(rx: &mut mpsc::UnboundedReceiver<Value>) -> Vec<Value> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut frames = Vec::new();
        while let Some(frame) = rx.recv().await {
            let terminal = frame["object"] == "response"
                && matches!(
                    frame["status"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                );
            frames.push(frame);
            if terminal {
                return frames;
            }
        }
        panic!("stream closed without terminal response");
    })
    .await
    .unwrap()
}
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn pdf(blank: bool) -> Vec<u8> {
    let stream = if blank {
        ""
    } else {
        "BT /F1 12 Tf 20 200 Td (Hello PDF family) Tj ET"
    };
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
    pdf.into_bytes()
}
fn xlsx() -> Vec<u8> {
    archive(&[
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
    ])
}

fn documents() -> Vec<(&'static str, Vec<u8>, &'static str, bool)> {
    let mut cases = vec![
        (
            "legacy.xls",
            include_bytes!("fixtures/attachments/date.xls").to_vec(),
            "44197\t15\n44198\t16",
            true,
        ),
        (
            "binary.xlsb",
            include_bytes!("fixtures/attachments/date.xlsb").to_vec(),
            "44197\t15\n44198\t16",
            true,
        ),
        (
            "macro.xlsm",
            include_bytes!("fixtures/attachments/issue221.xlsm").to_vec(),
            "Cell_A1\tCell_B1\nCell_A2\tCell_B2",
            true,
        ),
        ("report.pdf", pdf(false), "Hello PDF family", true),
        ("REPORT.PDF", pdf(false), "Hello PDF family", true),
        ("detected.bin", pdf(false), "Hello PDF family", true),
        (
            "家庭.docx",
            archive(&[(
                "word/document.xml",
                "<w:document><w:p><w:r><w:t>家庭 &amp; report</w:t></w:r></w:p></w:document>",
            )]),
            "家庭 & report",
            true,
        ),
        (
            "slides.pptx",
            archive(&[
                ("ppt/slides/slide10.xml", "<a:p><a:t>Last slide</a:t></a:p>"),
                ("ppt/slides/slide2.xml", "<a:p><a:t>First slide</a:t></a:p>"),
            ]),
            "First slide",
            true,
        ),
        ("book.xlsx", xlsx(), "你好\t2", true),
        ("book.xlsm", xlsx(), "你好\t2", true),
        (
            "book.ods",
            archive(&[
                ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
                (
                    "META-INF/manifest.xml",
                    r#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"/>"#,
                ),
                (
                    "content.xml",
                    r#"<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:spreadsheet><table:table table:name="家庭"><table:table-row><table:table-cell office:value-type="string"><text:p>ODS 家庭</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="42"/></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#,
                ),
            ]),
            "ODS 家庭\t42",
            true,
        ),
    ];
    for name in [
        "notes.txt",
        "notes.md",
        "data.csv",
        "data.tsv",
        "data.json",
        "data.xml",
        "page.html",
        "main.rs",
        "README",
    ] {
        cases.push((
            name,
            "UTF-8 家庭 & <data>\n42".as_bytes().to_vec(),
            "UTF-8 家庭 & <data>\n42",
            false,
        ));
    }
    cases
}

async fn roundtrip(
    name: &str,
    bytes: &[u8],
    expected: &str,
    extracted: bool,
    protocol: &str,
    via_path: bool,
) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("runtime");
    let runtime = Runtime::open(&root).unwrap();
    let reply = if protocol == "OpenAIResponseModel" {
        sse(json!({"type":"response.output_text.delta","delta":"Document accepted"}))
            + &sse(json!({"type":"response.completed","response":{"status":"completed"}}))
    } else {
        answer("Document accepted")
    };
    let (url, mut requests) = fixture(vec![("text/event-stream".into(), reply)]).await;
    configure(&runtime, &url, protocol).await;
    let path = tmp.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    let block = if via_path {
        potato_core::attachments::upload_path(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
    } else {
        let upload = runtime
            .request(
                "POST",
                "/api/console/upload",
                json!({"filename":name,"base64":STANDARD.encode(bytes)}),
            )
            .await
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(upload["original_file_name"], name);
        assert_eq!(upload["extracted"], extracted);
        let decoded = STANDARD
            .decode(upload["url"].as_str().unwrap().split_once(',').unwrap().1)
            .unwrap();
        assert_eq!(upload["size"], decoded.len());
        json!({"type":"file","file_url":upload["url"],"file_name":upload["file_name"]})
    };
    assert_eq!(
        block["file_name"],
        if extracted {
            format!("{name}.txt")
        } else {
            name.to_owned()
        }
    );
    let (tx, mut rx) = mpsc::unbounded_channel();
    runtime
        .start(
            "upload-run".into(),
            json!({"session_id":"attachment","input":[{"role":"user","content":[block.clone()]}]}),
            Arc::new(move |v| {
                let _ = tx.send(v);
                Ok(())
            }),
        )
        .unwrap();
    let frames = finish(&mut rx).await;
    assert_eq!(
        frames.last().unwrap()["status"],
        "completed",
        "{name}: {frames:?}"
    );
    let request = tokio::time::timeout(Duration::from_secs(5), requests.recv())
        .await
        .unwrap()
        .unwrap();
    let wire: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    let field = if protocol == "OpenAIResponseModel" {
        "input"
    } else {
        "messages"
    };
    let messages = wire[field].as_array().unwrap();
    let text = messages
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| {
            if let Some(s) = m["content"].as_str() {
                s.to_owned()
            } else {
                m["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains(expected), "{name} {protocol}: {text}");
    if via_path {
        let original_path = std::path::absolute(&path).unwrap();
        assert_eq!(block["original_path"], original_path.to_str().unwrap());
        assert_eq!(block["original_file_name"], name);
        assert!(text.contains(&format!(
            "original_path: {}",
            serde_json::to_string(&original_path).unwrap()
        )));
    } else {
        assert!(!text.contains("original_path:"));
    }
    assert!(!text.contains("data:text/plain;base64,"));
    assert!(
        text.contains(block["file_name"].as_str().unwrap()),
        "model lost attachment filename: {name}"
    );
    if name.ends_with("pptx") {
        assert!(text.find("First slide").unwrap() < text.find("Last slide").unwrap());
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "upload changed original"
    );
    drop(runtime);
    let reopened = Runtime::open(&root).unwrap();
    let chats = reopened
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let history = reopened
        .request(
            "GET",
            &format!("/api/chats/{}", chats[0]["id"].as_str().unwrap()),
            Value::Null,
        )
        .await
        .unwrap();
    let saved = history.to_string();
    assert!(
        saved.contains(block["file_url"].as_str().unwrap()),
        "missing preview: {name}"
    );
    assert!(
        saved.contains(block["file_name"].as_str().unwrap()),
        "missing filename: {name}"
    );
    assert!(saved.contains("Document accepted"), "missing reply: {name}");
}

#[tokio::test]
async fn document_formats_through_both_upload_routes_and_provider_protocols() {
    for (name, bytes, expected, extracted) in documents() {
        for protocol in ["OpenAIChatModel", "OpenAIResponseModel"] {
            for via_path in [false, true] {
                roundtrip(name, &bytes, expected, extracted, protocol, via_path).await;
            }
        }
    }
}

#[tokio::test]
async fn invalid_documents_are_rejected_by_both_upload_routes() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&tmp.path().join("runtime")).unwrap();
    let mut cases = vec![
        ("broken.pdf", b"%PDF-broken".to_vec(), 400),
        ("scanned.pdf", pdf(true), 415),
        ("binary.bin", vec![0xff, 0xfe, 0], 415),
        ("null.txt", b"hello\0world".to_vec(), 415),
        ("large.txt", vec![b'a'; 1_000_001], 413),
        (
            "traversal.docx",
            archive(&[("../word/document.xml", "<x/>")]),
            400,
        ),
        (
            "entity.docx",
            archive(&[(
                "word/document.xml",
                "<!DOCTYPE x SYSTEM 'file:///etc/passwd'><x/>",
            )]),
            400,
        ),
    ];
    for name in [
        "bad.docx", "bad.pptx", "bad.xlsx", "bad.xls", "bad.xlsb", "bad.xlsm", "bad.ods",
    ] {
        cases.push((name, b"not a document".to_vec(), 400));
    }
    for (name, bytes, status) in cases {
        let error = runtime
            .request(
                "POST",
                "/api/console/upload",
                json!({"filename":name,"base64":STANDARD.encode(&bytes)}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.status, status, "{name}: {error}");
        let path = tmp.path().join(name);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            potato_core::attachments::upload_path(&path)
                .unwrap_err()
                .status,
            status,
            "{name}"
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    for body in [
        json!({"filename":"bad.txt","base64":"!!!"}),
        json!({"filename":"../bad.txt","base64":"YQ=="}),
    ] {
        assert_eq!(
            runtime
                .request("POST", "/api/console/upload", body)
                .await
                .unwrap_err()
                .status,
            400
        );
    }
    let oversized = tmp.path().join("oversized.pdf");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(potato_core::attachments::MAX_BYTES + 1)
        .unwrap();
    assert_eq!(
        potato_core::attachments::upload_path(&oversized)
            .unwrap_err()
            .status,
        413
    );
    assert!(runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}
