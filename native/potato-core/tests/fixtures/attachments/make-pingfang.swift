import AppKit
let text = "季度研发报告\n\n第一章 概述\n本季度完成原生文件生成，验证中文内容。\n\n项目\t金额\t完成\n研发\t12000\t是\n测试\t3000\t否\n\nEnglish line for comparison: quarterly report."
let font = NSFont(name: "PingFang SC", size: 14) ?? NSFont.systemFont(ofSize: 14)
let attr = NSAttributedString(string: text, attributes: [.font: font])
var box = CGRect(x: 0, y: 0, width: 595, height: 842)
let ctx = CGContext(URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL, mediaBox: &box, nil)!
for page in 0..<2 {
  ctx.beginPDFPage(nil)
  let fs = CTFramesetterCreateWithAttributedString(page == 0 ? attr : NSAttributedString(string: "第二页：附录内容，用于检查分页。", attributes: [.font: font]))
  let path = CGPath(rect: box.insetBy(dx: 50, dy: 50), transform: nil)
  CTFrameDraw(CTFramesetterCreateFrame(fs, CFRange(location: 0, length: 0), path, nil), ctx)
  ctx.endPDFPage()
}
ctx.closePDF()
print("font used:", font.fontName)
