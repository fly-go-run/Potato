---
name: pptx
description: PowerPoint 演示文稿：原生生成 PPTX、可编辑图表、讲者备注及模板文本填充。
---

# PowerPoint 演示文稿

先用 read_skill(name="pptx", path="OFFICE.md") 阅读参数、图表示例与限制。
使用 create_office_file，format="pptx"，document={"title":"计划","slides":[{"title":"安排","bullets":["第一阶段","第二阶段"],"notes":"讲者备注"}]}。
图表支持 column、bar、line、pie，图表页 bullets 必须为空。categories 与各 series.values 长度须一致；饼图只有一个系列。
用 fill_office_template 填充项目内 PPTX 模板的文字占位符；不能修改母版或图片布局。
上传演示文稿会提取文本，可基于该内容分析，但不等于完整视觉读取。不要用 UTF-8 read_file 读取 PPTX 二进制文件。
输出到项目内的新文件，父目录须存在，遵循文件权限流程。原生工具没有页面渲染和视觉验收；保留密度警告，交付路径并说明需要在 PowerPoint 中检查排版。
