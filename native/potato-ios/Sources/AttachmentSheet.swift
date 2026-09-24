import SwiftUI
import PhotosUI
import AVFoundation
import UniformTypeIdentifiers

enum AttachmentDestination: Identifiable {
    case photos(UUID), files(UUID)
    var id: String { switch self { case .photos(let chat): "photos-\(chat)"; case .files(let chat): "files-\(chat)" } }
}

struct AttachmentSheet: View {
    @ObservedObject var store: WorkspaceStore
    @ObservedObject var imports: ComposerAttachments
    let chatID: UUID
    let openLibrary: () -> Void
    let openPhotos: () -> Void
    let openFiles: () -> Void
    @StateObject private var recent = RecentPhotos()
    @State private var showCamera = false
    @State private var showLimitedAccess = false
    @State private var failure: String?
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.dynamicTypeSize) private var typeSize
    private var chat: Conversation? { store.conversations.first { $0.id == chatID } }
    private var remaining: Int { chat.map { imports.remaining(in: $0) } ?? 0 }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack {
                    Text(L10n.tr("添加附件")).font(.title3.weight(.semibold))
                    Spacer()
                    Button { openPhotos(); dismiss() } label: {
                        Text(L10n.tr("所有照片")).font(.subheadline.weight(.medium))
                            .frame(minHeight: 44).contentShape(Rectangle())
                    }.disabled(remaining == 0).accessibilityIdentifier("attachment-all-photos")
                    Button { dismiss() } label: {
                        Image(systemName: "xmark").font(.system(size: 14, weight: .semibold))
                            .frame(width: 32, height: 32).background(Palette.ink.opacity(0.06), in: Circle()).frame(width: 44, height: 44).contentShape(Rectangle())
                    }.accessibilityLabel(L10n.tr("关闭")).accessibilityIdentifier("attachment-close")
                }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 20)

                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 10) {
                        Button(action: takePhoto) {
                            VStack(spacing: 10) { Image(systemName: "camera").font(.system(size: 26)); Text(L10n.tr("拍照")).font(.subheadline) }
                                .frame(width: 104, height: 112).background(Palette.ink.opacity(0.05), in: RoundedRectangle(cornerRadius: 20))
                        }.disabled(remaining == 0).accessibilityIdentifier("attachment-camera")
                        ForEach(recent.assets, id: \.localIdentifier) { asset in photoButton(asset) }
                        if recent.assets.isEmpty { photoAccess }
                    }.dynamicTypeSize(...DynamicTypeSize.xxxLarge).padding(.horizontal, 20)
                }.accessibilityIdentifier("attachment-recent-strip")

                if recent.authorization == .limited {
                    Button { showLimitedAccess = true } label: {
                        HStack { Text(L10n.tr("仅显示获准访问的照片")); Spacer(); Text(L10n.tr("管理")).fontWeight(.medium) }
                            .font(.footnote).frame(minHeight: 44).contentShape(Rectangle())
                    }.padding(.horizontal, 20).accessibilityIdentifier("attachment-manage-photos")
                }

                VStack(spacing: 0) {
                    actionRow(L10n.tr("选择文件"), icon: "doc.badge.plus", id: "attachment-files") { openFiles(); dismiss() }
                    Divider().padding(.leading, 54)
                    actionRow(L10n.tr("从资料库选择"), icon: "books.vertical", id: "attachment-library") { openLibrary(); dismiss() }
                }.background(Palette.surface, in: RoundedRectangle(cornerRadius: 22)).padding(.horizontal, 20)
                    .disabled(remaining == 0)

                HStack(alignment: .top) {
                    if remaining == 0 { Text(L10n.tr("每条消息最多添加 \(ComposerAttachments.limit) 个附件。")) }
                    Spacer(minLength: 8)
                    // The count only matters once something is selected.
                    if remaining < ComposerAttachments.limit {
                        Text(L10n.tr("已选 \(ComposerAttachments.limit - remaining) 个")).monospacedDigit()
                            .accessibilityLabel(L10n.tr("已选 \(ComposerAttachments.limit - remaining) 个附件"))
                            .accessibilityIdentifier("attachment-count")
                    }
                }.font(.footnote).foregroundStyle(Palette.secondary).padding(.horizontal, 24)

                if let error = failure ?? imports.failure(in: chatID) {
                    HStack(alignment: .top) {
                        Text(error).font(.footnote)
                        Spacer()
                        Button { failure = nil; imports.clearFailures(in: chatID) } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }.accessibilityLabel(L10n.tr("关闭"))
                    }.foregroundStyle(Palette.secondary).padding(.horizontal, 24).accessibilityIdentifier("attachment-error")
                }
            }.padding(.top, 18).padding(.bottom, 24)
        }
        .background(Palette.canvas).foregroundStyle(Palette.ink).buttonStyle(.plain)
        .presentationDetents(typeSize.isAccessibilitySize ? [.large] : [.height(recent.authorization == .limited ? 465 : 410), .large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(32)
        .accessibilityAction(.escape) { dismiss() }
        .task { recent.refresh() }
        .onChange(of: scenePhase) { _, phase in if phase == .active { recent.refresh() } }
        .fullScreenCover(isPresented: $showCamera) {
            AttachmentCamera { image in
                showCamera = false
                guard let image else { return }
                let storage = store.storage
                imports.add(to: chatID, store: store) {
                    try await Task.detached {
                        guard let data = image.jpegData(compressionQuality: 0.85) else { throw LocalFailure.message(L10n.tr("照片无法读取，请重新选择。")) }
                        return try storage.importData(data, name: L10n.tr("照片.jpg"), type: .image)
                    }.value
                }
            }.ignoresSafeArea()
        }
        .sheet(isPresented: $showLimitedAccess, onDismiss: { recent.refresh() }) {
            LimitedPhotoAccess { showLimitedAccess = false; recent.refresh() }
        }

    }

    private func actionRow(_ title: String, icon: String, id: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: icon).font(.system(size: 21)).frame(width: 26)
                Text(title).font(.body)
                Spacer()
                Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(Palette.secondary)
            }.padding(.horizontal, 18).padding(.vertical, 16).frame(minHeight: 56).contentShape(Rectangle())
        }.accessibilityIdentifier(id)
    }

    @ViewBuilder private var photoAccess: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(recent.authorization == .authorized || recent.authorization == .limited ? L10n.tr("暂无可显示的照片") : L10n.tr("快速添加最近照片"))
                .font(.subheadline.weight(.medium))
            if recent.authorization == .notDetermined {
                Button { Task { await recent.requestAccess() } } label: {
                    Text(L10n.tr("允许访问照片")).font(.subheadline.weight(.semibold)).padding(.horizontal, 14).frame(minHeight: 34)
                        .foregroundStyle(Palette.onInk).background(Palette.ink, in: Capsule()).frame(minHeight: 44).contentShape(Rectangle())
                }.accessibilityIdentifier("attachment-photo-permission")
            } else if recent.authorization == .denied {
                Button { if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) } } label: {
                    Text(L10n.tr("前往设置")).font(.subheadline.weight(.semibold)).padding(.horizontal, 14).frame(minHeight: 34)
                        .overlay { Capsule().stroke(Palette.line, lineWidth: 1) }.frame(minHeight: 44).contentShape(Rectangle())
                }.accessibilityIdentifier("attachment-photo-settings")
            } else {
                Text(L10n.tr("也可以从“所有照片”中选择。")).font(.footnote).foregroundStyle(Palette.secondary)
            }
        }.font(.subheadline).frame(width: 205, height: 112, alignment: .leading)
    }

    private func photoButton(_ asset: PHAsset) -> some View {
        let key = asset.localIdentifier
        let added = chat.flatMap { imports.attachment(for: key, in: $0) }
        let loading = imports.pending[chatID]?.contains(key) == true
        let failed = imports.failures[chatID]?[key] != nil
        let label = asset.creationDate?.formatted(date: .abbreviated, time: .shortened) ?? L10n.tr("照片")
        return Button {
            if let added {
                store.update(chatID) { $0.pendingAttachments.removeAll { $0.id == added } }; store.persist()
            } else {
                let storage = store.storage
                imports.add(key: key, to: chatID, store: store) {
                    let data = try await RecentPhotos.data(for: asset)
                    return try await Task.detached { try storage.importData(data, name: L10n.tr("照片.jpg"), type: .image) }.value
                }
            }
        } label: {
            RecentPhotoThumbnail(asset: asset).frame(width: 104, height: 112).clipped()
                .overlay(alignment: .topTrailing) {
                    if loading || added != nil || failed {
                        ZStack {
                            Circle().fill(.black.opacity(0.7)).frame(width: 28, height: 28)
                            if loading { ProgressView().tint(.white) }
                            else { Image(systemName: added != nil ? "checkmark" : "arrow.clockwise").font(.system(size: 14, weight: .bold)).foregroundStyle(.white) }
                        }.padding(7)
                    }
                }.clipShape(RoundedRectangle(cornerRadius: 20))
        }
        .disabled(loading || (remaining == 0 && added == nil))
        .accessibilityLabel(added != nil ? L10n.tr("移除照片，\(label)") : L10n.tr("添加照片，\(label)"))
        .accessibilityValue(loading ? L10n.tr("正在导入附件…") : added != nil ? L10n.tr("已添加") : failed ? L10n.tr("轻点重试") : "")
        .accessibilityIdentifier("attachment-recent-\(key)")
    }

    private func takePhoto() {
        guard UIImagePickerController.isSourceTypeAvailable(.camera) else { failure = L10n.tr("这台设备暂时无法使用相机，请从照片中选择。"); return }
        Task {
            var allowed = AVCaptureDevice.authorizationStatus(for: .video) == .authorized
            if !allowed { allowed = await AVCaptureDevice.requestAccess(for: .video) }
            if allowed { showCamera = true }
            else { failure = L10n.tr("请在系统设置中允许 Potato 使用相机。"); }
        }
    }
}

struct AttachmentPhotosPicker: UIViewControllerRepresentable {
    let limit: Int
    let finished: ([PHPickerResult]) -> Void
    func makeCoordinator() -> Coordinator { Coordinator(finished: finished) }
    func makeUIViewController(context: Context) -> PHPickerViewController {
        var configuration = PHPickerConfiguration(photoLibrary: .shared())
        configuration.filter = .images; configuration.selectionLimit = limit; configuration.selection = .ordered
        let picker = PHPickerViewController(configuration: configuration); picker.delegate = context.coordinator
        return picker
    }
    func updateUIViewController(_ controller: PHPickerViewController, context: Context) {}
    final class Coordinator: NSObject, PHPickerViewControllerDelegate {
        let finished: ([PHPickerResult]) -> Void
        init(finished: @escaping ([PHPickerResult]) -> Void) { self.finished = finished }
        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) { finished(results) }
    }
}

/// Explicit selection and cancellation return control to the owning presentation.
struct AttachmentDocumentPicker: UIViewControllerRepresentable {
    let finished: ([URL]?) -> Void
    func makeCoordinator() -> Coordinator { Coordinator(finished: finished) }
    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.text, .pdf, .image], asCopy: true)
        picker.allowsMultipleSelection = true; picker.delegate = context.coordinator
        return picker
    }
    func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}
    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let finished: ([URL]?) -> Void
        init(finished: @escaping ([URL]?) -> Void) { self.finished = finished }
        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { finished(nil) }
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) { finished(urls) }
    }
}

private struct AttachmentCamera: UIViewControllerRepresentable {
    let finished: (UIImage?) -> Void
    func makeCoordinator() -> Coordinator { Coordinator(finished: finished) }
    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController(); picker.sourceType = .camera; picker.delegate = context.coordinator; return picker
    }
    func updateUIViewController(_ controller: UIImagePickerController, context: Context) {}
    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let finished: (UIImage?) -> Void
        init(finished: @escaping (UIImage?) -> Void) { self.finished = finished }
        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { finished(nil) }
        func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) { finished(info[.originalImage] as? UIImage) }
    }
}
