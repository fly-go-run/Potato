import SwiftUI
import PhotosUI
import UniformTypeIdentifiers
import QuickLookThumbnailing

private enum LibraryCategory: String, CaseIterable { case all = "全部", images = "图片", documents = "文档" }
private enum LibrarySort: String, CaseIterable { case recent = "最近添加", name = "文件名", size = "文件大小" }
private struct LibraryShare: Identifiable { let id = UUID(); let urls: [URL] }

struct LibraryView: View {
    @ObservedObject var store: WorkspaceStore
    let openSidebar: () -> Void
    let used: () -> Void
    @State private var category = LibraryCategory.all
    @State private var sort = LibrarySort.recent
    @State private var search = ""
    @State private var scrollPositions: [LibraryCategory: UUID] = [:]
    @State private var path: [UUID] = []
    @State private var showTrash = false
    @State private var selecting = false
    @State private var selected = Set<UUID>()
    @State private var showFiles = false
    @State private var showPhotos = false
    @State private var photos: [PhotosPickerItem] = []
    @State private var showText = false
    @State private var share: LibraryShare?
    @State private var rename: LibraryItem?
    @State private var renameText = ""
    @State private var deletePermanently = false
    @State private var failure: String?
    @State private var progress: String?
    @State private var importTask: Task<Void, Never>?
    @FocusState private var searchFocused: Bool
    @Environment(\.dynamicTypeSize) private var typeSize
    private var matches: [LibraryItem] {
        store.library.filter { ($0.deletedAt != nil) == showTrash && $0.matches(search) }.sorted { a, b in
            switch sort {
            case .recent: return a.addedAt > b.addedAt
            case .name: return a.attachment.name.localizedStandardCompare(b.attachment.name) == .orderedAscending
            case .size: return a.attachment.size > b.attachment.size
            }
        }
    }
    /// Nothing to filter or search yet: the empty state carries the only add action.
    private var nothingSaved: Bool { !showTrash && !store.library.contains { $0.deletedAt == nil } }
    private var documents: [LibraryItem] { matches.filter { !$0.attachment.isImage } }
    private var images: [LibraryItem] { matches.filter { $0.attachment.isImage } }
    private var overview: Bool { category == .all && search.isEmpty && !selecting && !showTrash }
    private var empty: Bool { category == .images ? images.isEmpty : category == .documents ? documents.isEmpty : matches.isEmpty }
    var body: some View {
        NavigationStack(path: $path) {
            VStack(spacing: 0) {
                header.dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                if !nothingSaved { tabs.dynamicTypeSize(...DynamicTypeSize.xxxLarge) }
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 28) {
                        if let progress {
                            HStack { ProgressView(); Text(progress).font(.footnote); Spacer(); Button(L10n.tr("取消")) { importTask?.cancel() } }
                                .accessibilityIdentifier("library-import-progress")
                        }
                        if empty { emptyState }
                        if category != .images && !documents.isEmpty { documentSection }
                        if category != .documents && !images.isEmpty { imageSection }
                    }.padding(.horizontal, 20).padding(.top, 20).padding(.bottom, 24)
                }.scrollDismissesKeyboard(.interactively)
                    .scrollPosition(id: Binding(get: { scrollPositions[category] }, set: { scrollPositions[category] = $0 }), anchor: .top)
                    .safeAreaInset(edge: .bottom, spacing: 0) { if !nothingSaved { bottomBar.dynamicTypeSize(...DynamicTypeSize.xxxLarge) } }
            }.background(Palette.canvas).foregroundStyle(Palette.ink)
                .toolbar(.hidden, for: .navigationBar)
                .navigationDestination(for: UUID.self) { id in
                    LibraryFileDetail(store: store, itemID: id, used: used)
                }
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: LocalStorage.libraryTypes, allowsMultipleSelection: true) { result in
            switch result { case .success(let urls): importFiles(urls); case .failure(let error): failure = error.localizedDescription }
        }
        .photosPicker(isPresented: $showPhotos, selection: $photos, maxSelectionCount: 20, selectionBehavior: .ordered, matching: .images)
        .onChange(of: photos) { _, value in if !value.isEmpty { importPhotos(value) } }
        .sheet(isPresented: $showText) {
            LibraryTextEditor(store: store) { id in reveal(id) }
        }
        .sheet(item: $share) { ActivitySheet(items: $0.urls) }
        .alert(L10n.tr("无法完成操作"), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) {
            Button(L10n.tr("知道了"), role: .cancel) { failure = nil }
        } message: { Text(failure ?? "") }
        .alert(L10n.tr("重命名资料"), isPresented: Binding(get: { rename != nil }, set: { if !$0 { rename = nil } })) {
            TextField(L10n.tr("文件名"), text: $renameText)
            Button(L10n.tr("取消"), role: .cancel) { rename = nil }
            Button(L10n.tr("保存")) { if let rename { perform { try store.renameLibrary(rename.id, name: renameText) } }; rename = nil }
        }
        .confirmationDialog(L10n.tr("永久删除所选资料？此操作无法撤销。历史对话仍引用的附件会保留。"), isPresented: $deletePermanently, titleVisibility: .visible) {
            Button(L10n.tr("永久删除"), role: .destructive) { perform { try store.permanentlyDeleteLibrary(selected); selected = [] } }
        }
        .onChange(of: showTrash) { _, _ in selected = []; selecting = false; search = "" }
        .onChange(of: category) { _, _ in selected = [] }
        .onChange(of: search) { _, _ in selected = [] }
        .onDisappear { searchFocused = false }
        .onChange(of: store.error) { _, error in if let error { failure = error; store.error = nil } }
    }
    private var header: some View {
        HStack {
            Button { if showTrash { showTrash = false } else { searchFocused = false; openSidebar() } } label: {
                Image(systemName: showTrash ? "chevron.left" : "line.3.horizontal").font(.system(size: 21)).frame(width: 44, height: 44)
            }.chatGlass(in: Circle(), interactive: true).accessibilityLabel(showTrash ? L10n.tr("返回资料库") : L10n.tr("打开侧栏")).accessibilityIdentifier("library-sidebar")
            Spacer()
            Text(showTrash ? L10n.tr("最近删除") : L10n.tr("资料库")).font(.headline).accessibilityAddTraits(.isHeader)
            Spacer()
            if selecting {
                Button(L10n.tr("完成")) { selecting = false; selected = [] }.frame(minWidth: 44, minHeight: 44).accessibilityIdentifier("library-selection-done")
            } else {
                Menu {
                    Picker(L10n.tr("排序"), selection: $sort) { ForEach(LibrarySort.allCases, id: \.self) { Text(L10n.key($0.rawValue)).tag($0) } }
                    Button(L10n.tr("选择资料"), systemImage: "checkmark.circle") { selecting = true }.disabled(empty)
                    if !showTrash { Button(L10n.tr("最近删除"), systemImage: "trash") { showTrash = true } }
                } label: { Image(systemName: "ellipsis").font(.system(size: 21)).frame(width: 44, height: 44) }
                    .chatGlass(in: Circle(), interactive: true).accessibilityLabel(L10n.tr("资料库更多")).accessibilityIdentifier("library-more")
            }
        }.padding(.horizontal, 20).padding(.vertical, 8)
    }
    private var tabs: some View {
        HStack(spacing: 8) {
            ForEach(LibraryCategory.allCases, id: \.self) { tab in
                Button { category = tab } label: {
                    Text(L10n.key(tab.rawValue)).font(.body.weight(category == tab ? .semibold : .regular))
                        .foregroundStyle(category == tab ? Palette.ink : Palette.secondary)
                        .padding(.horizontal, 20).frame(minHeight: 44)
                        .background(category == tab ? Palette.muted : .clear, in: Capsule())
                }.accessibilityAddTraits(category == tab ? .isSelected : [])
                    .accessibilityIdentifier("library-category-\(tab.rawValue)")
            }
            Spacer(minLength: 0)
        }.padding(.horizontal, 16).padding(.top, 12).padding(.bottom, 4)
    }
    private func heading(_ title: String, count: Int, category target: LibraryCategory, limit: Int) -> some View {
        HStack {
            Text(title).font(.title3.weight(.semibold)).accessibilityAddTraits(.isHeader)
            Spacer()
            if overview && count > limit { Button { category = target } label: { HStack(spacing: 3) { Text(L10n.tr("查看全部")); Image(systemName: "chevron.right") }.font(.subheadline).foregroundStyle(Palette.secondary).frame(minHeight: 44) }.accessibilityLabel(L10n.tr("查看全部\(title)")) }
        }
    }
    private var documentSection: some View {
        VStack(spacing: 12) {
            heading(L10n.tr("文档"), count: documents.count, category: .documents, limit: 3)
            ForEach(overview ? Array(documents.prefix(3)) : documents) { item in
                HStack(spacing: 12) {
                    Button { open(item) } label: {
                        HStack(spacing: 14) {
                            if selecting { selectionMark(item) }
                            if !typeSize.isAccessibilitySize { LibraryThumbnail(item: item, storage: store.storage).frame(width: 80, height: 64).clipShape(RoundedRectangle(cornerRadius: 10)).accessibilityHidden(true) }
                            VStack(alignment: .leading, spacing: 6) {
                                Text(item.attachment.name).font(.body.weight(.medium)).lineLimit(typeSize.isAccessibilitySize ? nil : 2).multilineTextAlignment(.leading)
                                Text(metadata(item)).font(.caption).foregroundStyle(Palette.secondary)
                                if let snippet = item.excerpt(for: search) { Text(snippet).font(.caption).lineLimit(2).foregroundStyle(Palette.secondary) }
                            }
                            Spacer(minLength: 0)
                        }.contentShape(Rectangle())
                    }.buttonStyle(.plain).accessibilityIdentifier("library-file-\(item.attachment.name)")
                    if !selecting { Menu { actions(item) } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44).foregroundStyle(Palette.secondary) }.accessibilityLabel(L10n.tr("\(item.attachment.name)更多")) }
                }.padding(.vertical, 2).contextMenu { actions(item) }.id(item.id)
            }
        }.scrollTargetLayout()
    }
    private var imageSection: some View {
        VStack(spacing: 12) {
            heading(L10n.tr("图片"), count: images.count, category: .images, limit: 4)
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: typeSize.isAccessibilitySize ? 1 : 2), alignment: .leading, spacing: 18) {
                ForEach(overview ? Array(images.prefix(4)) : images) { item in
                    Button { open(item) } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            LibraryThumbnail(item: item, storage: store.storage).aspectRatio(1.2, contentMode: .fit).clipShape(RoundedRectangle(cornerRadius: 12))
                                .overlay(alignment: .topTrailing) { if selecting { selectionMark(item).padding(8).background(Palette.canvas, in: Circle()).padding(8) } }
                            Text(item.attachment.name).font(.subheadline.weight(.medium)).lineLimit(2)
                            Text(metadata(item)).font(.caption).foregroundStyle(Palette.secondary).lineLimit(2)
                        }.frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                    }.buttonStyle(.plain).contextMenu { actions(item) }.accessibilityIdentifier("library-file-\(item.attachment.name)").id(item.id)
                }
            }.scrollTargetLayout()
        }
    }
    private func metadata(_ item: LibraryItem) -> String {
        "\(item.format) · \(item.sizeLabel) · \(item.dateLabel)"
    }
    private func selectionMark(_ item: LibraryItem) -> some View {
        Image(systemName: selected.contains(item.id) ? "checkmark.circle.fill" : "circle").foregroundStyle(selected.contains(item.id) ? Color.blue : Palette.secondary)
            .accessibilityLabel(selected.contains(item.id) ? L10n.tr("已选择") : L10n.tr("未选择"))
    }
    private func open(_ item: LibraryItem) {
        searchFocused = false
        if selecting { if selected.contains(item.id) { selected.remove(item.id) } else { selected.insert(item.id) } }
        else { path.append(item.id) }
    }
    @ViewBuilder private func actions(_ item: LibraryItem) -> some View {
        if showTrash {
            Button(L10n.tr("恢复"), systemImage: "arrow.uturn.backward") { perform { try store.trashLibrary([item.id], restore: true) } }
            Button(L10n.tr("永久删除"), systemImage: "trash", role: .destructive) { selected = [item.id]; deletePermanently = true }
        } else {
            Button(L10n.tr("重命名"), systemImage: "pencil") { renameText = item.attachment.name; rename = item }
            Button(L10n.tr("分享"), systemImage: "square.and.arrow.up") { shareFiles([item.id]) }
            Button(L10n.tr("移到最近删除"), systemImage: "trash", role: .destructive) { perform { try store.trashLibrary([item.id]) } }
        }
    }
    private var emptyState: some View {
        VStack(spacing: 14) {
            Image(systemName: search.isEmpty ? (showTrash ? "trash" : "doc.on.doc") : "magnifyingglass").font(.system(size: 36)).foregroundStyle(Palette.secondary)
            Text(search.isEmpty ? (showTrash ? L10n.tr("最近删除为空") : nothingSaved ? L10n.tr("还没有资料") : category == .images ? L10n.tr("还没有图片") : category == .documents ? L10n.tr("还没有文档") : L10n.tr("还没有资料")) : L10n.tr("没有找到相关资料")).font(.title3.weight(.semibold))
            if !search.isEmpty { Button(L10n.tr("清除搜索")) { search = "" } }
            else if nothingSaved { addMenu(label: true) }
        }.frame(maxWidth: .infinity).padding(.vertical, 72).accessibilityElement(children: .contain).accessibilityIdentifier("library-empty")
    }
    private var bottomBar: some View {
        VStack(spacing: 8) {
            if selecting {
                Text(L10n.tr("已选择 \(selected.count) 项")).font(.caption).foregroundStyle(Palette.secondary)
                HStack {
                    if showTrash {
                        Button(L10n.tr("恢复")) { perform { try store.trashLibrary(selected, restore: true); selected = [] } }
                        Spacer()
                        Button(L10n.tr("永久删除"), role: .destructive) { deletePermanently = true }
                    } else {
                        Menu(L10n.tr("用于对话")) {
                            Button(L10n.tr("当前对话")) { useSelection(newChat: false) }
                            Button(L10n.tr("新对话")) { useSelection(newChat: true) }
                        }
                        Spacer()
                        Button { shareFiles(selected) } label: { Image(systemName: "square.and.arrow.up").frame(width: 48, height: 48).contentShape(Rectangle()) }.buttonStyle(.plain).accessibilityLabel(L10n.tr("分享所选资料"))
                        Spacer()
                        Button(role: .destructive) { perform { try store.trashLibrary(selected); selected = [] } } label: { Image(systemName: "trash").frame(width: 48, height: 48).contentShape(Rectangle()) }.buttonStyle(.plain).accessibilityLabel(L10n.tr("删除所选资料"))
                    }
                }.frame(minHeight: 48).padding(.horizontal, 20).chatGlass(in: Capsule()).disabled(selected.isEmpty)
            } else {
                HStack(spacing: 12) {
                    HStack(spacing: 8) {
                        Image(systemName: "magnifyingglass").foregroundStyle(Palette.secondary)
                        TextField(L10n.tr("搜索资料"), text: $search).focused($searchFocused).submitLabel(.search).onSubmit { searchFocused = false }.accessibilityIdentifier("library-search")
                        if !search.isEmpty { Button { search = "" } label: { Image(systemName: "xmark.circle.fill") }.accessibilityLabel(L10n.tr("清除搜索")) }
                    }.padding(.horizontal, 16).frame(minHeight: 52).chatGlass(in: Capsule())
                    if searchFocused { Button(L10n.tr("取消")) { searchFocused = false; search = "" }.frame(minHeight: 44) }
                    else if !showTrash { addMenu(label: false) }
                }
            }
        }.padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 12)
    }
    private func addMenu(label: Bool) -> some View {
        Menu {
            Button(L10n.tr("从照片选择"), systemImage: "photo") { showPhotos = true }
            Button(L10n.tr("从文件导入"), systemImage: "folder") { showFiles = true }
            Button(L10n.tr("粘贴文本"), systemImage: "doc.on.clipboard") { showText = true }
        } label: {
            if label { Text(L10n.tr("添加资料")).padding(.horizontal, 22).frame(minHeight: 48).foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule()) }
            else { Image(systemName: "plus").font(.system(size: 26)).frame(width: 52, height: 52).chatGlass(in: Circle(), interactive: true) }
        }.disabled(progress != nil).accessibilityLabel(L10n.tr("添加资料")).accessibilityIdentifier("library-add")
    }
    private func perform(_ action: () throws -> Void) { do { try action() } catch { failure = error.localizedDescription } }
    private func shareFiles(_ ids: Set<UUID>) { perform { share = LibraryShare(urls: try store.library.filter { ids.contains($0.id) }.map { try store.storage.libraryShareURL($0.attachment) }) } }
    private func useSelection(newChat: Bool) { perform { try store.useLibrary(selected, newConversation: newChat); selecting = false; selected = []; used() } }
    private func reveal(_ id: UUID) {
        if let item = store.library.first(where: { $0.id == id }) { category = item.attachment.isImage ? .images : .documents }
        search = ""; sort = .recent; path = [id]
    }
    private func importFiles(_ urls: [URL]) {
        let storage = store.storage
        progress = L10n.tr("准备导入…")
        importTask = Task { @MainActor in
            defer { progress = nil; importTask = nil }
            var last: UUID?, errors: [String] = []
            for (index, url) in urls.enumerated() {
                if Task.isCancelled { break }
                progress = L10n.tr("正在导入 \(index + 1) / \(urls.count)")
                do {
                    let file = try await Task.detached { try storage.importLibraryFile(url) }.value
                    try Task.checkCancellation()
                    last = try store.saveLibraryFile(file)
                } catch is CancellationError { break }
                catch { errors.append("\(url.lastPathComponent)：\(error.localizedDescription)") }
            }
            if !errors.isEmpty { failure = errors.joined(separator: "\n") }
            else if !Task.isCancelled, let last { reveal(last) }
        }
    }
    private func importPhotos(_ values: [PhotosPickerItem]) {
        let storage = store.storage
        progress = L10n.tr("正在读取照片…")
        importTask = Task { @MainActor in
            defer { progress = nil; importTask = nil; photos = [] }
            var last: UUID?, errors: [String] = []
            for (index, photo) in values.enumerated() {
                if Task.isCancelled { break }
                progress = L10n.tr("正在导入 \(index + 1) / \(values.count)")
                do {
                    guard let data = try await photo.loadTransferable(type: Data.self) else { throw LocalFailure.message(L10n.tr("照片无法读取，请重试。")) }
                    try Task.checkCancellation()
                    let type = photo.supportedContentTypes.first(where: { $0.conforms(to: .image) }) ?? .jpeg
                    let file = try await Task.detached { try storage.importLibraryData(data, name: L10n.tr("照片 \(Date().formatted(.dateTime.month().day().locale(AppLocalization.shared.locale))) \(index + 1).\(type.preferredFilenameExtension ?? "jpg")"), type: type) }.value
                    try Task.checkCancellation(); last = try store.saveLibraryFile(file)
                } catch is CancellationError { break }
                catch { errors.append(error.localizedDescription) }
            }
            if !errors.isEmpty { failure = errors.joined(separator: "\n") }
            else if !Task.isCancelled, let last { reveal(last) }
        }
    }
}

struct LibraryThumbnail: View {
    let item: LibraryItem
    let storage: LocalStorage
    @State private var image: UIImage?
    @State private var missing = false
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Palette.surface
                if let image {
                    Image(uiImage: image).resizable().aspectRatio(contentMode: item.attachment.isImage ? .fill : .fit)
                        .frame(width: geometry.size.width, height: geometry.size.height)
                } else if missing {
                    Image(systemName: "exclamationmark.doc").foregroundStyle(Palette.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let text = item.attachment.extractedText, !text.isEmpty {
                    Text(String(text.prefix(500))).font(.system(size: 9)).foregroundStyle(Palette.ink).padding(8)
                } else {
                    let kind = FileKind(filename: item.attachment.name, type: item.attachment.type)
                    Image(systemName: kind.symbol).font(.title2).foregroundStyle(kind.tint).frame(maxWidth: .infinity, maxHeight: .infinity).background(kind.tint.opacity(0.08))
                }
            }.frame(width: geometry.size.width, height: geometry.size.height).clipped()
        }.task(id: item.attachment.filename) {
            let url = storage.url(for: item.attachment)
            missing = !FileManager.default.fileExists(atPath: url.path)
            guard !missing else { return }
            if item.attachment.extractedText != nil && item.attachment.type != "application/pdf" { return }
            let request = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: 360, height: 360), scale: 2, representationTypes: .thumbnail)
            let thumbnail = try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: request)
            if !Task.isCancelled { image = thumbnail?.uiImage }
        }.accessibilityHidden(true)
    }
}

private struct LibraryTextEditor: View {
    @ObservedObject var store: WorkspaceStore
    let saved: (UUID) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var text = ""
    @State private var failure: String?
    var body: some View {
        NavigationStack {
            Form {
                TextField(L10n.tr("名称（可选）"), text: $name).accessibilityIdentifier("library-text-name")
                Section {
                    TextEditor(text: $text).frame(minHeight: 240).accessibilityIdentifier("library-text-content")
                    PasteButton(payloadType: String.self) { text = $0.joined(separator: "\n") }
                }
            }.scrollDismissesKeyboard(.interactively).navigationTitle(L10n.tr("保存文本")).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button(L10n.tr("取消")) { dismiss() } }
                    ToolbarItem(placement: .confirmationAction) { Button(L10n.tr("保存")) {
                        do { let id = try store.saveLibraryText(text, name: name); dismiss(); saved(id) }
                        catch { failure = error.localizedDescription }
                    }.disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("library-text-save") }
                }
                .alert(L10n.tr("无法保存"), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) { Button(L10n.tr("知道了"), role: .cancel) {} } message: { Text(failure ?? "") }
        }
    }
}
