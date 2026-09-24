import SwiftUI

/// A light band that sweeps across in-progress text. Static with Reduce Motion.
private struct Shimmer: ViewModifier {
    var active: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let period = 2.0, sweep = 1.4
    func body(content: Content) -> some View {
        if active && !reduceMotion {
            content.overlay {
                TimelineView(.animation) { context in
                    GeometryReader { geo in
                        let t = min(1, context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: period) / sweep)
                        let band = max(40, geo.size.width * 0.35)
                        LinearGradient(colors: [.clear, Palette.ink, .clear], startPoint: .leading, endPoint: .trailing)
                            .frame(width: band)
                            .offset(x: -band + (geo.size.width + band) * t)
                    }
                }.mask(content).allowsHitTesting(false).accessibilityHidden(true)
            }
        } else { content }
    }
}

extension View {
    func shimmering(_ active: Bool = true) -> some View { modifier(Shimmer(active: active)) }
}
