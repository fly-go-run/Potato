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

/// Shown after sending, before anything of the reply has arrived.
struct ReplyPendingDot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let phase = reduceMotion ? 1 : (sin(context.date.timeIntervalSinceReferenceDate * .pi / 0.7) + 1) / 2
            Circle().fill(Palette.ink)
                .frame(width: 12, height: 12)
                .scaleEffect(0.75 + 0.25 * phase)
                .opacity(0.45 + 0.55 * phase)
        }.frame(width: 24, height: 24, alignment: .leading)
    }
}
