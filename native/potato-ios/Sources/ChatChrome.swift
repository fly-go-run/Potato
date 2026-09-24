import SwiftUI

// Keep the system material on the control layer, with a readable fallback before iOS 26.
private struct ChatGlassSurface<S: InsettableShape>: ViewModifier {
    let shape: S
    var interactive = false
    @Environment(\.accessibilityReduceTransparency) private var systemReduceTransparency
    private var reduceTransparency: Bool {
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-testing") && ProcessInfo.processInfo.arguments.contains("--reduce-transparency-preview") { return true }
        #endif
        return systemReduceTransparency
    }

    @ViewBuilder func body(content: Content) -> some View {
        if reduceTransparency {
            content.background(Palette.canvas, in: shape)
                .overlay { shape.strokeBorder(Palette.line, lineWidth: 0.5).allowsHitTesting(false) }
        } else if #available(iOS 26.0, *) {
            // Let the system own the lens, edge highlights and shadow.
            content.glassEffect(.regular.interactive(interactive), in: shape)
        } else {
            content.background(.regularMaterial, in: shape)
                .overlay { shape.strokeBorder(Palette.glassEdge, lineWidth: 0.5).allowsHitTesting(false) }
                .shadow(color: .black.opacity(0.04), radius: 8, y: 3)
        }
    }
}

// Confine the readability scrim to the status area. A full toolbar-height scroll
// edge effect washes out the content before it reaches the floating glass.
struct ChatStatusBackdrop: View {
    var body: some View {
        Rectangle().fill(.ultraThinMaterial)
            .mask {
                LinearGradient(stops: [.init(color: .black, location: 0),
                                       .init(color: .black, location: 0.65),
                                       .init(color: .clear, location: 1)],
                               startPoint: .top, endPoint: .bottom)
            }
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}

extension View {
    func chatGlass<S: InsettableShape>(in shape: S, interactive: Bool = false) -> some View {
        modifier(ChatGlassSurface(shape: shape, interactive: interactive))
    }

    @ViewBuilder func chatBar<Bar: View>(edge: VerticalEdge, @ViewBuilder content: () -> Bar) -> some View {
        if #available(iOS 26.0, *) {
            safeAreaBar(edge: edge, spacing: 0, content: content)
        } else {
            safeAreaInset(edge: edge, spacing: 0, content: content)
        }
    }

    /// Text scrolling under the floating header fades softly instead of colliding with the buttons;
    /// the composer edge keeps no effect so the last lines stay crisp.
    @ViewBuilder func chatScrollEdges(_ edges: Edge.Set = .vertical) -> some View {
        if #available(iOS 26.0, *) {
            scrollEdgeEffectStyle(.soft, for: .top).scrollEdgeEffectHidden(true, for: edges.subtracting(.top))
        } else {
            self
        }
    }
}
