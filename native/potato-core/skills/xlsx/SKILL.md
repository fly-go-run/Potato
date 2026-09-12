---
name: xlsx
description: Excel 表格：原生生成 XLSX，支持有限公式计算、表头与数字格式，分析上传表格。
---

# Excel 表格

先用 read_skill(name="xlsx", path="OFFICE.md") 阅读参数与限制。
使用 create_office_file，format="xlsx"，例如：

```json
{"file_path":"budget.xlsx","format":"xlsx","document":{"sheets":[{"name":"预算","header":true,"rows":[["项目","金额"],["研发",12000],["测试",3000],["合计",{"formula":"=SUM(B2:B3)"}]]}]}}
```

普通字符串按字面值写入；公式须显式写为 {"formula":"=SUM(B2:B3)"}。
支持有限的大写函数 SUM、AVERAGE、MIN、MAX、COUNT、ABS、ROUND 及本工作表范围内的数值运算。由 Rust 引擎计算真实缓存值，错误会拒绝输出。
不支持跨表引用、宏、任意已有工作簿重算或编辑。用户上传表格可提取内容用于分析；不要用 UTF-8 read_file 读取 XLSX 二进制文件。
输出必须是项目内的新文件，父目录须存在。保持文件权限流程，报告工具返回的路径和计算状态，不声称已视觉检查。
