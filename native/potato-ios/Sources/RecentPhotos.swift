import SwiftUI
import Photos
import PhotosUI

@MainActor
final class RecentPhotos: NSObject, ObservableObject, PHPhotoLibraryChangeObserver {
    @Published private(set) var authorization = PHPhotoLibrary.authorizationStatus(for: .readWrite)
    @Published private(set) var assets: [PHAsset] = []
    private var observing = false

    func refresh() {
        authorization = PHPhotoLibrary.authorizationStatus(for: .readWrite)
        guard authorization == .authorized || authorization == .limited else { assets = []; return }
        if !observing { PHPhotoLibrary.shared().register(self); observing = true }
        let options = PHFetchOptions()
        options.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
        options.fetchLimit = 30
        let result = PHAsset.fetchAssets(with: .image, options: options)
        var photos: [PHAsset] = []
        result.enumerateObjects { asset, _, _ in photos.append(asset) }
        assets = photos
    }
    func requestAccess() async {
        _ = await PHPhotoLibrary.requestAuthorization(for: .readWrite)
        refresh()
    }
    nonisolated func photoLibraryDidChange(_ changeInstance: PHChange) {
        Task { @MainActor [weak self] in self?.refresh() }
    }
    deinit { if observing { PHPhotoLibrary.shared().unregisterChangeObserver(self) } }
}

/// One terminal callback, including a bounded wait for photos stored in iCloud.
@MainActor
private final class PhotoDataRequest {
    private var continuation: CheckedContinuation<Data, Error>?
    private var requestID: PHImageRequestID?
    private var timeout: Task<Void, Never>?

    func load(_ asset: PHAsset) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            self.continuation = continuation
            let options = PHImageRequestOptions()
            options.isNetworkAccessAllowed = true
            options.deliveryMode = .highQualityFormat
            options.version = .current
            requestID = PHImageManager.default().requestImageDataAndOrientation(for: asset, options: options) { data, _, _, info in
                let error = info?[PHImageErrorKey] as? Error
                let cancelled = (info?[PHImageCancelledKey] as? Bool) == true
                Task { @MainActor in
                    if let error { self.finish(.failure(error)) }
                    else if cancelled { self.finish(.failure(CancellationError())) }
                    else if let data { self.finish(.success(data)) }
                    else { self.finish(.failure(LocalFailure.message(L10n.tr("照片无法读取，请重新选择。")))) }
                }
            }
            timeout = Task {
                do { try await Task.sleep(for: .seconds(60)) } catch { return }
                self.finish(.failure(LocalFailure.message(L10n.tr("照片下载超时，请检查网络后轻点重试。"))))
            }
        }
    }
    private func finish(_ result: Result<Data, Error>) {
        guard let continuation else { return }
        self.continuation = nil
        timeout?.cancel(); timeout = nil
        if let requestID { PHImageManager.default().cancelImageRequest(requestID) }
        requestID = nil
        continuation.resume(with: result)
    }
}

extension RecentPhotos {
    static func data(for asset: PHAsset) async throws -> Data { try await PhotoDataRequest().load(asset) }
}

struct RecentPhotoThumbnail: View {
    let asset: PHAsset
    @State private var image: UIImage?
    @State private var requestID: PHImageRequestID?
    var body: some View {
        ZStack {
            Palette.ink.opacity(0.05)
            if let image { Image(uiImage: image).resizable().scaledToFill() }
            else { Image(systemName: "icloud").font(.title2).foregroundStyle(Palette.secondary) }
        }
        .onAppear {
            let options = PHImageRequestOptions()
            options.deliveryMode = .opportunistic
            // Scrolling thumbnails never downloads full originals.
            options.isNetworkAccessAllowed = false
            requestID = PHImageManager.default().requestImage(for: asset, targetSize: CGSize(width: 300, height: 300), contentMode: .aspectFill, options: options) { value, _ in
                if let value { Task { @MainActor in image = value } }
            }
        }
        .onDisappear { if let requestID { PHImageManager.default().cancelImageRequest(requestID) }; requestID = nil }
        .accessibilityHidden(true)
    }
}

struct LimitedPhotoAccess: UIViewControllerRepresentable {
    var finished: () -> Void
    func makeUIViewController(context: Context) -> Controller { let controller = Controller(); controller.finished = finished; return controller }
    func updateUIViewController(_ controller: Controller, context: Context) {}
    final class Controller: UIViewController {
        var finished: (() -> Void)?
        private var presented = false
        override func viewDidAppear(_ animated: Bool) {
            super.viewDidAppear(animated)
            guard !presented else { return }; presented = true
            PHPhotoLibrary.shared().presentLimitedLibraryPicker(from: self) { _ in
                DispatchQueue.main.async { self.finished?() }
            }
        }
    }
}
