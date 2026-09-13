import SwiftUI
import UIKit

/// One direction decision per touch sequence. A vertical start can never turn
/// into opening the drawer halfway through scrolling a conversation.
struct SidebarDrag {
    enum Axis { case undecided, horizontal, rejected }
    let startedOpen: Bool
    let width: CGFloat
    private(set) var axis: Axis = .undecided
    private(set) var translation: CGFloat = 0

    init(open: Bool, width: CGFloat, allowed: Bool) {
        startedOpen = open; self.width = max(1, width)
        if !allowed { axis = .rejected }
    }
    mutating func update(_ delta: CGSize) {
        if axis == .undecided {
            guard max(abs(delta.width), abs(delta.height)) >= 10 else { return }
            if abs(delta.width) > abs(delta.height) * 1.35 && (startedOpen || delta.width > 0) { axis = .horizontal }
            else { axis = .rejected }
        }
        if axis == .horizontal { translation = delta.width }
    }
    var active: Bool { axis == .horizontal }
    var offset: CGFloat { min(width, max(0, (startedOpen ? width : 0) + translation)) }
    func destination(predicted: CGFloat) -> Bool {
        guard active else { return startedOpen }
        // Use a bounded velocity projection: a short accidental movement must
        // not jump an entire drawer, but a deliberate flick can finish it.
        let momentum = min(width * 0.35, max(-width * 0.35, predicted - translation))
        let projected = offset + momentum
        return projected > width * (startedOpen ? 0.60 : 0.40)
    }
}

final class SidebarGestureRegistry: ObservableObject {
    private final class Area { weak var view: UIView?; init(_ view: UIView) { self.view = view } }
    private var areas: [Area] = []
    func register(_ view: UIView) {
        areas.removeAll { $0.view == nil || $0.view === view }; areas.append(Area(view))
    }
    func remove(_ view: UIView) { areas.removeAll { $0.view == nil || $0.view === view } }
    func contains(_ point: CGPoint, in window: UIWindow) -> Bool {
        areas.contains { area in
            guard let view = area.view, view.window === window, !view.isHidden else { return false }
            return view.convert(view.bounds, to: window).contains(point)
        }
    }
}
private struct SidebarRegistryKey: EnvironmentKey { static var defaultValue: SidebarGestureRegistry? { nil } }
extension EnvironmentValues {
    var sidebarGestureRegistry: SidebarGestureRegistry? {
        get { self[SidebarRegistryKey.self] }
        set { self[SidebarRegistryKey.self] = newValue }
    }
}
private struct SidebarExclusionMarker: UIViewRepresentable {
    @Environment(\.sidebarGestureRegistry) private var registry
    final class Coordinator { weak var registry: SidebarGestureRegistry? }
    func makeCoordinator() -> Coordinator { Coordinator() }
    func makeUIView(context: Context) -> UIView {
        let view = UIView(); view.isUserInteractionEnabled = false; view.isAccessibilityElement = false
        return view
    }
    func updateUIView(_ view: UIView, context: Context) {
        context.coordinator.registry?.remove(view)
        context.coordinator.registry = registry; registry?.register(view)
    }
    static func dismantleUIView(_ view: UIView, coordinator: Coordinator) { coordinator.registry?.remove(view) }
}
extension View {
    /// Editors, dictation and horizontal content keep ownership of their pans.
    func excludesSidebarGesture() -> some View {
        background(SidebarExclusionMarker())
    }
}

/// Reject a vertical touch before UIKit begins a pan. Unlike a simultaneous
/// SwiftUI DragGesture, a recognized native pan cancels the underlying row tap.
private final class SidebarPanRecognizer: UIPanGestureRecognizer {
    var startedOpen = false
    private var origin: CGPoint?
    private var horizontal = false
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        if origin == nil { origin = touches.first?.location(in: view) }
        super.touchesBegan(touches, with: event)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        if state == .possible, !horizontal, let origin, let point = touches.first?.location(in: view) {
            let dx = point.x - origin.x, dy = point.y - origin.y
            if max(abs(dx), abs(dy)) >= 8 {
                guard abs(dx) > abs(dy) * 1.35, startedOpen || dx > 0 else { state = .failed; return }
                horizontal = true
            }
        }
        super.touchesMoved(touches, with: event)
    }
    override func reset() { origin = nil; horizontal = false; super.reset() }
}

struct SidebarPanBridge: UIViewRepresentable {
    let enabled: Bool
    let open: Bool
    let registry: SidebarGestureRegistry
    let changed: (CGSize) -> Void
    let ended: (CGFloat, Bool) -> Void

    final class ScopeView: UIView {
        var windowChanged: (() -> Void)?
        override func didMoveToWindow() { super.didMoveToWindow(); windowChanged?() }
    }
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeUIView(context: Context) -> ScopeView {
        let view = ScopeView(); view.isUserInteractionEnabled = false; view.isAccessibilityElement = false
        context.coordinator.scope = view
        view.windowChanged = { [weak coordinator = context.coordinator] in coordinator?.install() }
        return view
    }
    func updateUIView(_ view: ScopeView, context: Context) {
        context.coordinator.parent = self
        context.coordinator.install()
    }
    static func dismantleUIView(_ view: ScopeView, coordinator: Coordinator) {
        view.windowChanged = nil; coordinator.detach()
    }
    final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var parent: SidebarPanBridge
        weak var scope: UIView?
        private weak var installedWindow: UIWindow?
        private lazy var pan: SidebarPanRecognizer = {
            let recognizer = SidebarPanRecognizer(target: self, action: #selector(handlePan(_:)))
            recognizer.delegate = self; recognizer.maximumNumberOfTouches = 1
            recognizer.cancelsTouchesInView = true
            return recognizer
        }()
        init(_ parent: SidebarPanBridge) { self.parent = parent }
        func install() {
            if installedWindow !== scope?.window {
                detach(); installedWindow = scope?.window; installedWindow?.addGestureRecognizer(pan)
            }
            pan.isEnabled = parent.enabled
            if pan.state == .possible { pan.startedOpen = parent.open }
        }
        func detach() { installedWindow?.removeGestureRecognizer(pan); installedWindow = nil }
        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
            guard parent.enabled, let scope, let window = scope.window, let touched = touch.view else { return false }
            let point = touch.location(in: window)
            guard scope.convert(scope.bounds, to: window).contains(point), !parent.registry.contains(point, in: window) else { return false }
            // A presented sheet/menu is not part of the workspace controller.
            var responder: UIResponder? = scope
            while responder != nil && !(responder is UIViewController) { responder = responder?.next }
            guard let controller = responder as? UIViewController, touched.isDescendant(of: controller.view) else { return false }
            var ancestor: UIView? = touched
            while let current = ancestor, current !== window {
                if current is UITextView || current is UITextField || current is UISlider { return false }
                if let scroll = current as? UIScrollView,
                   scroll.alwaysBounceHorizontal || scroll.contentSize.width > scroll.bounds.width + 1 { return false }
                ancestor = current.superview
            }
            pan.startedOpen = parent.open
            return true
        }
        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldBeRequiredToFailBy otherGestureRecognizer: UIGestureRecognizer) -> Bool {
            // Vertical scrolling waits only for the first axis decision; it
            // starts normally as soon as this recognizer rejects a vertical pan.
            otherGestureRecognizer is UIPanGestureRecognizer && otherGestureRecognizer.view is UIScrollView
                && !(otherGestureRecognizer is UIScreenEdgePanGestureRecognizer)
        }
        @objc private func handlePan(_ gesture: UIPanGestureRecognizer) {
            let translation = gesture.translation(in: installedWindow)
            switch gesture.state {
            case .began, .changed: parent.changed(CGSize(width: translation.x, height: translation.y))
            case .ended: parent.ended(translation.x + gesture.velocity(in: installedWindow).x * 0.16, false)
            case .cancelled, .failed: parent.ended(0, true)
            default: break
            }
        }
    }
}

/// Place inside scroll content. Observe the existing scroll pan without adding
/// another recognizer that competes with navigation's interactive back gesture.
struct ScrollActivityObserver: UIViewRepresentable {
    var onVerticalDrag: () -> Void
    final class ObserverView: UIView {
        var onVerticalDrag: () -> Void = {}
        private weak var observedPan: UIPanGestureRecognizer?
        private var notified = false
        override func didMoveToWindow() { super.didMoveToWindow(); observeScrollView() }
        override func didMoveToSuperview() { super.didMoveToSuperview(); observeScrollView() }
        override func layoutSubviews() { super.layoutSubviews(); observeScrollView() }
        func detach() {
            observedPan?.removeTarget(self, action: #selector(scrolled(_:))); observedPan = nil
        }
        private func observeScrollView() {
            guard window != nil else { detach(); return }
            var ancestor = superview
            while ancestor != nil && !(ancestor is UIScrollView) { ancestor = ancestor?.superview }
            let pan = (ancestor as? UIScrollView)?.panGestureRecognizer
            guard observedPan !== pan else { return }
            detach(); observedPan = pan
            pan?.addTarget(self, action: #selector(scrolled(_:)))
        }
        @objc private func scrolled(_ pan: UIPanGestureRecognizer) {
            if pan.state == .began { notified = false }
            guard !notified, pan.state == .began || pan.state == .changed else { return }
            let translation = pan.translation(in: pan.view)
            if abs(translation.y) > abs(translation.x) {
                notified = true; onVerticalDrag()
            }
        }
    }
    func makeUIView(context: Context) -> ObserverView {
        let view = ObserverView(); view.isUserInteractionEnabled = false; view.isAccessibilityElement = false
        view.onVerticalDrag = onVerticalDrag
        return view
    }
    func updateUIView(_ view: ObserverView, context: Context) { view.onVerticalDrag = onVerticalDrag }
    static func dismantleUIView(_ view: ObserverView, coordinator: ()) { view.detach() }
}
