# GPUI rich-text typography patch

Vendored gpui-base 0.6.0 from crates.io (original licenses retained).
Only src/text/node.rs is modified. Cargo patches this dependency so clean
builds use the same rendering, without modifying the developer Cargo cache.

Changes apply consistently to native rich-text views:
- List markers occupy at least 2 rem, right aligned with 0.75 rem trailing space.
  Wide ordered markers can grow. Continuations and nesting use 2 rem indentation.
- Heading trailing space is 0.5 rem.
- Horizontal rules are 1 px; 1 rem before (in addition to the preceding block's
  usual 1 rem) and 2 rem after. No extra gap between tight list items.

Parser, source spans, text selection, copy and streaming behavior are unchanged.
When upgrading GPUI, rebase these small rendering changes or replace them with
upstream style hooks. Do not copy the old lockfile/checksum metadata back in.
