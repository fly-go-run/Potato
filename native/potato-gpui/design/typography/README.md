# Web-reference typography

Scope: native GPUI + vendored gpui-base 0.6.0 rich-text rendering.

Reference: user-supplied ChatGPT web screenshot, 3320 × 2032 pixels.
Measured body row pitch was 52 source-image pixels, with 1536-pixel column width.
At the Potato 16-unit body size this suggested 26 line height and 768 width;
these are screenshot-derived targets, not claims about the website CSS.

- Body: platform font, 16 / 26; column and composer 768 max width.
- List: 32-unit marker column (minimum for wide ordered markers), 12 trailing
  units between the marker column's text and body. Tight lists have no extra gap.
- Continuation paragraphs, blocks and nested lists indent by 32.
- Task checkboxes plus their trailing margin also occupy 32.
- Headings: 24 / 21 / 18 / 16, with 8 trailing units.
- Rules: 1 unit, 16 before plus preceding block gap, 32 after.

See [vendor notes](../../vendor/README.md) for dependency patch maintenance. The patch applies to native rich-text views consistently,
including tool details/file Markdown. It does not change parsed or copied source.

web-reference.json is a manually transcribed VISUAL FIXTURE from the screenshot.
Its factual claims and substitute link targets are not verified research.
preview.jpg is the actual native preview of the same excerpt.

Validation: debug build passes. 65 GPUI tests pass with --test-threads=1.
Parallel run: 62 pass, 3 fail in window interaction tests; serial rerun passes.
Visual inspection: nested line alignment, width, list markers and separator spacing.
