import SwiftUI
import QuickLook

struct AttachmentPreview: View {
    let attachments: [Attachment]
    let storage: LocalStorage
    @State var index: Int
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            PreviewController(attachments: attachments, storage: storage, index: $index)
                .navigationTitle(attachments.count > 1 ? "\(index + 1) / \(attachments.count)" : attachments[index].name).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) { Button("完成") { dismiss() }.accessibilityIdentifier("close-preview") }
                    ToolbarItemGroup(placement: .bottomBar) {
                        if attachments.count > 1 {
                            Button { index -= 1 } label: { Image(systemName: "chevron.left").frame(width: 44, height: 44) }.disabled(index == 0).accessibilityLabel("上一张图片").accessibilityIdentifier("previous-image")
                            Text("\(index + 1) / \(attachments.count)").monospacedDigit().accessibilityIdentifier("image-position")
                            Button { index += 1 } label: { Image(systemName: "chevron.right").frame(width: 44, height: 44) }.disabled(index == attachments.count - 1).accessibilityLabel("下一张图片").accessibilityIdentifier("next-image")
                        }
                        Spacer()
                        ShareLink(item: storage.url(for: attachments[index])) { Image(systemName: "square.and.arrow.up").frame(width: 44, height: 44) }.accessibilityLabel("分享当前附件")
                    }
                }
        }
    }
}
private struct PreviewController: UIViewControllerRepresentable {
    let attachments: [Attachment]
    let storage: LocalStorage
    @Binding var index: Int
    func makeCoordinator() -> Coordinator { Coordinator(items: attachments.map { Item(url: storage.url(for: $0), name: $0.name) }, index: $index) }
    func makeUIViewController(context: Context) -> QLPreviewController {
        let controller = QLPreviewController(); controller.dataSource = context.coordinator; controller.delegate = context.coordinator
        controller.currentPreviewItemIndex = index
        context.coordinator.observation = controller.observe(\.currentPreviewItemIndex, options: [.new]) { _, change in
            if let value = change.newValue, value >= 0, value < attachments.count { DispatchQueue.main.async { self.index = value } }
        }
        return controller
    }
    func updateUIViewController(_ controller: QLPreviewController, context: Context) { if controller.currentPreviewItemIndex != index { controller.currentPreviewItemIndex = index } }
    final class Coordinator: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
        let items: [Item]
        var index: Binding<Int>
        var observation: NSKeyValueObservation?
        init(items: [Item], index: Binding<Int>) { self.items = items; self.index = index }
        func numberOfPreviewItems(in controller: QLPreviewController) -> Int { items.count }
        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> any QLPreviewItem { items[index] }
    }
    final class Item: NSObject, QLPreviewItem {
        var previewItemURL: URL?
        var previewItemTitle: String?
        init(url: URL, name: String) { previewItemURL = url; previewItemTitle = name }
    }
}
