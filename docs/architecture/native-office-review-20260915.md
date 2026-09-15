# 内置 Office 能力复核与首批修复

范围：GPUI → potato-core 原生调用链。2026-09-15 对照当前源码及本机
Cargo 缓存中的锁定版本复核。没有修改并行工作的 `skills.rs`、`mcp.rs`、
`tool_registry.rs`，也没有扩展旧客户端。

## 审查结论

| 项目 | 复核结果与处理 |
| --- | --- |
| PDF 中文部首错码 | 成立。将 CoreText/PingFang 两页样例加入回归夹具；提取后的“第一章”“原生文件”“第二页”恢复为普通汉字。已修复。 |
| PDF 分页 | 成立。已增加 `--- Page N ---`，保留中间、首尾空白页的页码。全空文档仍返回需要 OCR。 |
| 附件原路径 | 成立。`upload_path` 添加 `original_path`、`original_file_name`，消息转换保留 JSON 引号包裹的路径；旧 base64 上传不虚构本地路径。已修复。 |
| XLSX 日期、公式读取 | 成立。当前 `Data::to_string()` 将日期写成序列值，未使用 `worksheet_formula`。后续应输出有单元格地址的公式及缓存值，日期需区分 1900/1904 日期系统、时间与时长；现有 xls/xlsb 夹具可扩展。 |
| XLSX 公式开放 | 当前确实只允许七个函数；IronCalc 的枚举包含 494 项及报告提到的 IF、SUMIF、VLOOKUP、XLOOKUP、TEXT、DATE。但仅换黑名单、放开引号不完整：还要处理比较运算符、文本和布尔结果的 OOXML 缓存类型、错误结果及求值资源限制。当前 `CellValue::Number` 分支仍拒绝 IF 返回“是/否”。暂不扩大求值范围。 |
| DOCX blocks | 建议成立。当前 schema 只有 title、paragraphs；docx-rs 支持表格、段落样式、编号等。属于新功能，应兼容旧结构，补真实标题样式、列表编号、表格尺寸和图片资源边界。暂未实现。 |
| PPTX 图片、表格 | 建议成立，但不是直接加两个字段即可。`with_table()` 仅设置标志，具体数据使用 `.table(Table)`；图片 API 同时有文件和 URL 读取路线，适配层需要继续约束为经过授权的内存资源，并保留图表替换和包校验。暂未实现。 |
| XLSX 图表和格式 | 上游有能力；仍需定义图表数据范围、位置、样式及对应测试。当前 PPT 图表会生成独立内嵌工作簿，不能原封不动作为 XLSX 工作表图表复用。暂未实现。 |
| macOS 预览 | Quick Look 首页缩略图可辅助发现明显问题，不能据此宣称整份 Office 文档通过视觉验证；`textutil` 的格式转换也不是分页渲染。这轮未新增系统命令调用或修改技能描述。 |

## 修复设计中的调整

1. **限定部首归一化。** 康熙部首使用现有 unicode-normalization 的 NFKC；
   CJK Radicals Supplement 使用 [Unicode 17.0 等价汉字映射](https://www.unicode.org/Public/17.0.0/ucd/EquivalentUnifiedIdeograph.txt)。
   不对全文做 NFKC，避免改写 `①`、`²`、全角字符、单位符号等内容。
   无等价映射的字符保留。真实讨论部首字形的 PDF 仍存在语义歧义，原 PDF 保持不变。
2. **不直接使用 by_pages 便利函数。** pdf-extract 0.12 的实现用
   `while let Ok(...)` 结束循环，可能将页错误当作结束。现在使用已锁定的
   lopdf 0.42 加载一次，枚举实际页码，通过 `output_doc_page` 提取并传播错误；
   保留 panic 隔离及 1 MB 提取文本上限。lopdf 从传递依赖提升为直接依赖，
   关闭默认特性，没有新增 crate 或版本升级。
3. **路径是数据，不是授权。** 路径在模型消息中被 JSON 引号包裹，校验为
   非 NUL、长度受限的绝对路径。`lib.rs` 仅替换附件消息格式化调用。
   `fill_office_template` 仍限制普通、非符号链接的项目内文件；
   这次解决模型不知道路径的问题，不代表项目外上传模板可直接填充。

## 验证

- 附件单元回归：9 项通过，包括真实 PingFang PDF、部首与兼容字符、空白页、
  后续页解析失败、原路径的编码和校验。
- 附件端到端：2 项测试通过，其中成功矩阵包含 20 文档 × 两种上传入口 ×
  两种模型协议，共 80 组合；原路径确实进入模型 HTTP 请求，源文件保持不变。
- GPUI `cargo +1.96.1 check --offline` 通过。
- Core library Clippy（`-D warnings`）通过，修改文件 diff 无空白错误。
- 测试使用本机伪模型服务；未做真实模型理解评估、GPUI 文件选择器交互、
  Windows 实机或 Office 逐页视觉验收。扫描件 OCR、PDF 生成/编辑未增加。

复跑入口：

```sh
cargo +1.96.1 test --locked --offline --manifest-path native/potato-core/Cargo.toml --lib attachments::
cargo +1.96.1 test --locked --offline --manifest-path native/potato-core/Cargo.toml --test attachments_e2e
cargo +1.96.1 check --locked --offline --manifest-path native/potato-gpui/Cargo.toml
```
