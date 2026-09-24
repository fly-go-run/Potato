import SwiftUI
import QuickLook

struct LibraryFileDetail: View {
    @ObservedObject var store: WorkspaceStore
    let itemID: UUID
    let used: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var rename = false
    @State private var name = ""
    @State private var failure: String?
    @State private var showInfo = false
    @State private var showDelete = false
    @State private var shareURL: URL?
    @State private var sharing = false
    @State private var fullText: String?
    @State private var preparing = false
    private var item: LibraryItem? { store.library.first { $0.id == itemID } }
    var body: some View {
        Group {
            if let item { content(item) }
            else { ContentUnavailableView(L10n.tr("资料已移除"), systemImage: "doc", description: Text(L10n.tr("请返回资料库重新选择。"))) }
        }.background(Palette.canvas).foregroundStyle(Palette.ink).excludesSidebarGesture()
            .navigationBarTitleDisplayMode(.inline).toolbar(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    if let item {
                        Menu {
                            Button(L10n.tr("文件信息"), systemImage: "info.circle") { showInfo = true }
                            Button(L10n.tr("分享"), systemImage: "square.and.arrow.up") { perform { shareURL = try store.storage.libraryShareURL(item.attachment); sharing = true } }
                            if item.deletedAt == nil {
                                Button(L10n.tr("重命名"), systemImage: "pencil") { name = item.attachment.name; rename = true }
                                Button(L10n.tr("移到最近删除"), systemImage: "trash", role: .destructive) { perform { try store.trashLibrary([itemID]); dismiss() } }
                            } else {
                                Button(L10n.tr("恢复"), systemImage: "arrow.uturn.backward") { perform { try store.trashLibrary([itemID], restore: true); dismiss() } }
                                Button(L10n.tr("永久删除"), systemImage: "trash", role: .destructive) { showDelete = true }
                            }
                        } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }.accessibilityLabel(L10n.tr("文件更多")).accessibilityIdentifier("library-detail-more")
                    }
                }
            }
            .sheet(isPresented: $sharing) { if let shareURL { ActivitySheet(items: [shareURL]) } }
            .sheet(isPresented: $showInfo) { information }
            .alert(L10n.tr("重命名资料"), isPresented: $rename) {
                TextField(L10n.tr("文件名"), text: $name)
                Button(L10n.tr("取消"), role: .cancel) {}
                Button(L10n.tr("保存")) { perform { try store.renameLibrary(itemID, name: name) } }
            }
            .alert(L10n.tr("无法完成操作"), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) { Button(L10n.tr("知道了"), role: .cancel) {} } message: { Text(failure ?? "") }
            .confirmationDialog(L10n.tr("永久删除此资料？此操作无法撤销。历史对话仍引用的附件会保留。"), isPresented: $showDelete, titleVisibility: .visible) {
                Button(L10n.tr("永久删除"), role: .destructive) { perform { try store.permanentlyDeleteLibrary([itemID]); dismiss() } }
            }
            .task(id: itemID) {
                guard let item, !item.attachment.isImage, item.attachment.type != "application/pdf",
                      item.attachment.extractedText != nil else { return }
                let url = store.storage.url(for: item.attachment)
                fullText = await Task.detached(priority: .userInitiated) { try? String(contentsOf: url, encoding: .utf8) }.value
            }
    }
    private func content(_ item: LibraryItem) -> some View {
        VStack(spacing: 12) {
            Text(item.attachment.name).font(.title3.weight(.semibold)).textSelection(.enabled).accessibilityIdentifier("library-detail-title")
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 20)
            if !FileManager.default.fileExists(atPath: store.storage.url(for: item.attachment).path) {
                ContentUnavailableView(L10n.tr("文件已不可用"), systemImage: "exclamationmark.doc", description: Text(L10n.tr("请返回资料库重新导入原文件。")))
            } else if let fullText {
                ScrollView { Text(fullText).font(.body).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(20) }.accessibilityIdentifier("library-text-preview")
            } else if QLPreviewController.canPreview(store.storage.url(for: item.attachment) as NSURL) {
                PreviewController(attachments: [item.attachment], storage: store.storage, index: .constant(0)).id(item.attachment.name).accessibilityIdentifier("library-native-preview")
            } else {
                ContentUnavailableView(L10n.tr("此格式暂不支持预览"), systemImage: "doc", description: Text(L10n.tr("可以分享到其他应用打开。")))
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
            .safeAreaInset(edge: .bottom) {
                VStack(spacing: 10) {
                    if item.deletedAt != nil {
                        Button(L10n.tr("恢复资料")) { perform { try store.trashLibrary([itemID], restore: true); dismiss() } }.buttonStyle(.borderedProminent)
                    } else {
                        if let reason = store.storage.libraryChatLimitation(item.attachment) { Text(reason).font(.caption).foregroundStyle(Palette.secondary).multilineTextAlignment(.center) }
                        HStack(spacing: 10) {
                            Button { use(newChat: false) } label: {
                                HStack { if preparing { ProgressView().tint(Palette.onInk) }; Text(L10n.tr("用于对话")).font(.body.weight(.semibold)) }.frame(maxWidth: .infinity, minHeight: 50)
                            }.foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule()).accessibilityIdentifier("library-use")
                            Menu { Button(L10n.tr("用于新对话")) { use(newChat: true) } } label: { Image(systemName: "chevron.down").frame(width: 48, height: 48).chatGlass(in: Circle()) }.accessibilityLabel(L10n.tr("选择目标对话"))
                        }.disabled(preparing || store.storage.libraryChatLimitation(item.attachment) != nil)
                    }
                }.padding(.horizontal, 20).padding(.vertical, 12).background(Palette.canvas).dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            }
    }
    private var information: some View {
        NavigationStack {
            List {
                if let item {
                    LabeledContent(L10n.tr("名称"), value: item.attachment.name)
                    LabeledContent(L10n.tr("格式"), value: item.format)
                    LabeledContent(L10n.tr("大小"), value: item.sizeLabel)
                    LabeledContent(L10n.tr("添加时间"), value: item.addedAt.formatted(.dateTime.locale(AppLocalization.shared.locale)))
                    if !item.sources.isEmpty {
                        Section(L10n.tr("来源对话")) {
                            ForEach(item.sources, id: \.self) { source in
                                if let chat = store.conversations.first(where: { $0.id == source.conversationID && $0.deletedAt == nil }) {
                                    Button(chat.title) { showInfo = false; store.select(chat.id); store.recallFocusID = source.messageID; used() }
                                } else { Text(L10n.tr("来源对话已删除或不可用")).foregroundStyle(Palette.secondary) }
                            }
                        }
                    }
                }
            }.navigationTitle(L10n.tr("文件信息")).navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("完成")) { showInfo = false } } }
        }
    }
    private func perform(_ action: () throws -> Void) { do { try action() } catch { failure = error.localizedDescription } }
    private func use(newChat: Bool) {
        preparing = true
        Task { @MainActor in
            defer { preparing = false }
            await Task.yield()
            perform { try store.useLibrary([itemID], newConversation: newChat); used() }
        }
    }
}
