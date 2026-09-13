import SwiftUI
import ImageIO

struct AttachmentThumbnail: View {
    let url: URL
    @State private var image: UIImage?
    var body: some View {
        GeometryReader { geometry in
            Group {
                if let image { Image(uiImage: image).resizable().scaledToFill() }
                else { ZStack { Palette.muted; Image(systemName: "photo").foregroundStyle(Palette.secondary) } }
            }.frame(width: geometry.size.width, height: geometry.size.height).clipped()
        }.task(id: url) {
            let loaded = await Task.detached(priority: .utility) {
                guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
                      let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 600, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return UIImage?.none }
                return UIImage(cgImage: cg)
            }.value
            if !Task.isCancelled { image = loaded }
        }.accessibilityHidden(true)
    }
}

struct MessageImageGrid: View {
    let attachments: [Attachment]
    let storage: LocalStorage
    let preview: (Attachment) -> Void
    var body: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 5), count: attachments.count == 1 ? 1 : 2), spacing: 5) {
            ForEach(Array(attachments.enumerated()), id: \.element.id) { index, attachment in
                Button { preview(attachment) } label: {
                    AttachmentThumbnail(url: storage.url(for: attachment))
                        .frame(height: attachments.count == 1 ? 200 : 126)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                }.buttonStyle(.plain).accessibilityLabel("预览 \(attachment.name)，第 \(index + 1) 张，共 \(attachments.count) 张")
                    .accessibilityIdentifier("message-image-\(index)")
            }
        }.frame(maxWidth: 300)
    }
}
