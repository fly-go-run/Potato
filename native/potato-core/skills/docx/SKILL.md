---
name: docx
description: Word 文档：原生生成 DOCX、填充模板占位符，分析上传文档的文本。
---

# Word 文档

使用 Rust 原生工具。先用 read_skill(name="docx", path="OFFICE.md") 阅读参数、示例和限制。

- 新建：create_office_file，format="docx"，document={"title":"报告","paragraphs":["正文"]}。
- 填充现有项目模板：fill_office_template，format="docx"，提供 template_path、file_path 和 replacements 映射。支持同段落内跨文本 run 的占位符。
- 阅读：用户上传 DOCX 后，附件管线提取文本。按实际收到的内容分析；缺失正文时请用户上传文件。read_file 只读取 UTF-8，不能用它读取 Office 二进制文件。

输出路径相对于当前项目，父目录须存在，不能覆盖现有文件。遵循正常文件权限与审批流程。
不支持任意排版编辑、图片替换、批注修订或原生 PDF 导出。不把文本提取当作完整布局解析，不声称已完成视觉验证。返回工具提供的文件路径及实际警告。
