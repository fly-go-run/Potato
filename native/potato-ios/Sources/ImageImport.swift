import Foundation
import ImageIO
import UniformTypeIdentifiers
import CryptoKit

// Decode a bounded thumbnail instead of expanding a full-resolution photo in memory.
enum ImageImport {
    // E2B may return both a displayed PNG and the same pixels saved as a file.
    // Compare full pixels, never resized thumbnails; skip large/animated/oriented images.
    static func pngPixelDigest(_ data: Data) -> String? {
        guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetType(source) as String? == UTType.png.identifier,
              CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 4096, height <= 4096, width * height <= 4_194_304,
              (properties[kCGImagePropertyOrientation] as? Int ?? 1) == 1,
              let image = CGImageSourceCreateImageAtIndex(source, 0, [kCGImageSourceShouldCache: false] as CFDictionary),
              let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
        var pixels = Data(count: width * height * 4)
        let drawn = pixels.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: colorSpace, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return nil }
        return "\(width)x\(height):" + SHA256.hash(data: pixels).map { String(format: "%02x", $0) }.joined()
    }

    static func jpeg(from data: Data, maxBytes: Int = 600_000) throws -> Data {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { throw LocalFailure.message("照片无法读取，请重新选择。") }
        for dimension in [1600, 1280, 960, 640, 480, 320] {
          guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: dimension,
                kCGImageSourceShouldCacheImmediately: true
              ] as CFDictionary) else { throw LocalFailure.message("照片无法读取，请重新选择。") }
        let result = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(result, UTType.jpeg.identifier as CFString, 1, nil) else { throw LocalFailure.message("照片无法转换。") }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.75] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { throw LocalFailure.message("照片转换失败。") }
          if result.length <= maxBytes { return result as Data }
        }
        throw LocalFailure.message("图片无法压缩到发送限制内，请裁剪后重试。")
    }
}
