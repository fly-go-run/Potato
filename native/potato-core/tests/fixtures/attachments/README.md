# Attachment end-to-end fixtures

These three unmodified binary fixtures come from tafia/calamine, downloaded
2026-09-06 under the MIT license (see LICENSE-MIT.md). Tests embed these bytes
and require no network access or office applications.

- `date.xls`: https://raw.githubusercontent.com/tafia/calamine/master/tests/date.xls
  Git blob: `95831ba90fdcec90f27bbc2bbf9cb63f398e65f9`; SHA-256: `e3eb3391a371cc1e0b85d94b9f1dd94bbcedf19733ed0ca449e2d269af758e9e`.
- `date.xlsb`: https://raw.githubusercontent.com/tafia/calamine/master/tests/date.xlsb
  Git blob: `4a206a2633ea32109d584363e2d34d27b00870b7`; SHA-256: `68d36adf4d4de3890209e786eeb89a98778d25158628f83e96587e652fbb4eca`.
- `issue221.xlsm`: https://raw.githubusercontent.com/tafia/calamine/master/tests/issue221.xlsm
  Git blob: `18f68a6eb6e0596cc022aadfe2ff72cc91704b9e`; SHA-256: `4757c73be0ad3e0cb789dcf6c170df7ddf10160618986af41614278dc8ce6498`.

PDF, DOCX, PPTX, XLSX, ODS and text samples are generated directly in Rust.
The additional `pingfang.pdf` regression fixture was generated with CoreText
and PingFang SC on macOS using `make-pingfang.swift` (2026-09-15). It contains
two pages of synthetic Chinese text. Reproduce with
`swift make-pingfang.swift pingfang.pdf` on macOS; normal tests embed the PDF
and run on all platforms without Swift or installed PingFang fonts.
The generated XLSM case exercises extension routing; issue221.xlsm supplies a
real macro-enabled workbook container. Macros are never executed.

Run from the repository root:

```sh
cargo test --manifest-path native/potato-core/Cargo.toml --test attachments_e2e --offline
```

The success matrix runs 20 document cases through both upload APIs (native file
path and base64 runtime request) and both provider protocols (Chat Completions
and Responses): 80 end-to-end combinations. It checks extracted content,
filenames, original-file preservation, HTTP request content, terminal SSE
status, and attachment/reply history after reopening the runtime. The local
HTTP fixture requires permission to bind a loopback port. It does not evaluate
an actual model's comprehension or operate the graphical file picker.

Failure cases cover corrupt PDF/Office/spreadsheet files, a valid PDF with no
text (OCR-required response), invalid UTF-8, embedded NUL, extracted-text limits,
archive traversal, XML document types, invalid base64/filenames, and a sparse
file larger than the upload limit. Both upload routes must reject bad documents
without changing original bytes or creating chats.
