#if DEBUG
import SwiftUI
import PDFKit

enum LibraryPreviewFixtures {
    @MainActor static func prepare(_ store: WorkspaceStore) {
        let arguments = ProcessInfo.processInfo.arguments
        guard arguments.contains("--ui-testing"), arguments.contains("--library-fixtures"), arguments.contains("--reset") else { return }
        do {
            store.library = []
            store.newChat()
            store.update { $0.title = "资料库验收"; $0.input = "保留的草稿" }
            let report = UIGraphicsPDFRenderer(bounds: CGRect(x: 0, y: 0, width: 400, height: 540)).pdfData { context in
                context.beginPage()
                ("推理优化报告" as NSString).draw(at: CGPoint(x: 30, y: 50), withAttributes: [.font: UIFont.boldSystemFont(ofSize: 26)])
                ("大语言模型推理性能优化与实践\n\n架构、吞吐量与延迟\n\n2026 年 9 月" as NSString).draw(in: CGRect(x: 30, y: 100, width: 330, height: 350), withAttributes: [.font: UIFont.systemFont(ofSize: 18)])
            }
            _ = try store.saveLibraryFile(store.storage.importLibraryData(report, name: "推理优化报告.pdf", type: .pdf))
            _ = try store.saveLibraryText("# 产品设计笔记\n\n记录产品设计过程中的思考。\n\n- 清晰的资料分类\n- 保留文件原件\n- 随时用于对话", name: "产品设计笔记.md")
            _ = try store.saveLibraryText("# 项目说明\n\n资料库保存文档、图片和文本。验收关键词：独立留存。", name: "项目说明.txt")
            // Real bundled image bytes are only synthetic test content, never production seeds.
            if let image = UIImage(named: "PotatoMark"), let data = image.pngData() {
                _ = try store.saveLibraryFile(store.storage.importLibraryData(data, name: "Potato 图标.png", type: .png))
            }
            store.persist()
        } catch { store.error = error.localizedDescription }
    }
}
#endif
