import SwiftUI
import UIKit

// One native editor supplies sizing, UTF-16 selection and IME handling in both surfaces.
struct ComposerTextInput: UIViewRepresentable {
    @Binding var text: String
    @Binding var selection: NSRange?
    var focused: Binding<Bool>
    let placeholder: String
    var minimumHeight: CGFloat = 0
    var maximumHeight: CGFloat = 180
    var fillsAvailableHeight = false
    var readOnly = false
    var followsInsertion = true
    var insertionRange: NSRange?
    var followRevision = 0
    var identifier = "composer-input"
    var onTap: (() -> Void)?
    var onOverflow: (Bool) -> Void = { _ in }
    var onManualScroll: () -> Void = {}

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeUIView(context: Context) -> UITextView {
        let view = UITextView(); view.delegate = context.coordinator
        view.backgroundColor = .clear; view.textContainerInset = UIEdgeInsets(top: 2, left: 0, bottom: 2, right: 0)
        view.textContainer.lineFragmentPadding = 0
        view.adjustsFontForContentSizeCategory = true; view.font = .preferredFont(forTextStyle: .body)
        view.textColor = UIColor(Palette.ink); view.tintColor = UIColor(Palette.ink)
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.tapTranscript))
        tap.delegate = context.coordinator; tap.cancelsTouchesInView = false; view.addGestureRecognizer(tap)
        view.isScrollEnabled = fillsAvailableHeight
        return view
    }
    func updateUIView(_ view: UITextView, context: Context) {
        let coordinator = context.coordinator
        coordinator.parent = self; coordinator.updating = true
        let changedText = view.text != text && view.markedTextRange == nil
        if changedText {
            let offset = view.contentOffset
            view.text = text
            if readOnly && !followsInsertion {
                DispatchQueue.main.async { [weak view, weak coordinator] in
                    guard let view, let coordinator, !coordinator.parent.followsInsertion else { return }
                    view.setContentOffset(offset, animated: false)
                }
            }
        }
        view.isEditable = !readOnly
        if readOnly { view.font = UIFont.preferredFont(forTextStyle: .body); view.textColor = UIColor(Palette.ink); view.accessibilityHint = L10n.tr("轻点结束录音并修改文字") }
        view.accessibilityIdentifier = identifier; view.accessibilityLabel = placeholder
        if !readOnly, (changedText || !view.isFirstResponder), let selection, Range(selection, in: text) != nil, view.selectedRange != selection {
            view.selectedRange = selection
        }
        if !readOnly && focused.wrappedValue && !view.isFirstResponder { view.becomeFirstResponder() }
        else if (!focused.wrappedValue || readOnly) && view.isFirstResponder { view.resignFirstResponder() }
        if readOnly && followsInsertion && (changedText || coordinator.lastRevision != followRevision || coordinator.lastInsertion != insertionRange) {
            if coordinator.lastRevision != followRevision { view.setContentOffset(view.contentOffset, animated: false) }
            coordinator.reveal = insertionRange ?? NSRange(location: (text as NSString).length, length: 0)
        } else if !readOnly && changedText { coordinator.reveal = view.selectedRange }
        if readOnly && !followsInsertion { coordinator.reveal = nil }
        coordinator.lastRevision = followRevision; coordinator.lastInsertion = insertionRange
        coordinator.updating = false
        coordinator.scheduleReveal(view)
    }
    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
        guard let width = proposal.width else { return nil }
        let natural = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
        let line = (uiView.font?.lineHeight ?? 22) + 4
        let maximum = fillsAvailableHeight ? max(line, proposal.height ?? maximumHeight) : max(line, maximumHeight)
        let minimum = min(maximum, max(line, minimumHeight))
        let height = fillsAvailableHeight ? maximum : max(minimum, min(maximum, natural))
        uiView.isScrollEnabled = natural > height + 0.5 || fillsAvailableHeight
        let overflow = natural > height + 0.5
        if context.coordinator.lastOverflow != overflow {
            context.coordinator.lastOverflow = overflow
            DispatchQueue.main.async { context.coordinator.parent.onOverflow(overflow) }
        }
        context.coordinator.scheduleReveal(uiView)
        return CGSize(width: width, height: height)
    }
    final class Coordinator: NSObject, UITextViewDelegate, UIGestureRecognizerDelegate {
        var parent: ComposerTextInput; var updating = false
        var lastOverflow: Bool?; var lastRevision = -1; var lastInsertion: NSRange?
        var reveal: NSRange?; private var revealScheduled = false
        init(_ parent: ComposerTextInput) { self.parent = parent }
        @objc func tapTranscript() { parent.onTap?() }
        func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool { parent.readOnly && parent.onTap != nil }
        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool { true }
        func scheduleReveal(_ view: UITextView) {
            guard reveal != nil, !revealScheduled else { return }
            revealScheduled = true
            DispatchQueue.main.async { [weak self, weak view] in
                guard let self, let view else { return }; self.revealScheduled = false
                guard let range = self.reveal else { return }; self.reveal = nil
                guard !view.isDragging, Range(range, in: view.text) != nil else { return }
                view.layoutManager.ensureLayout(for: view.textContainer)
                view.layoutIfNeeded()
                if NSMaxRange(range) == (view.text as NSString).length {
                    let bottom = max(0, view.contentSize.height - view.bounds.height + view.adjustedContentInset.bottom)
                    view.setContentOffset(CGPoint(x: 0, y: bottom), animated: false)
                } else if let position = view.position(from: view.beginningOfDocument, offset: NSMaxRange(range)) {
                    view.scrollRectToVisible(view.caretRect(for: position).insetBy(dx: 0, dy: -4), animated: false)
                }
            }
        }
        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text; parent.selection = textView.selectedRange
            textView.invalidateIntrinsicContentSize()
        }
        func textViewDidChangeSelection(_ textView: UITextView) {
            if !updating && !parent.readOnly { parent.selection = textView.selectedRange }
        }
        func textViewDidBeginEditing(_ textView: UITextView) { if !updating { parent.focused.wrappedValue = true } }
        func textViewDidEndEditing(_ textView: UITextView) { if !updating { parent.focused.wrappedValue = false } }
        func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
            if parent.readOnly { reveal = nil; parent.onManualScroll() }
        }
    }
}

struct ExpandedComposer: View {
    @Binding var text: String
    @Binding var selection: NSRange?
    let canSend: Bool
    let collapse: () -> Void
    let send: () -> Void
    @State private var focused = true
    var body: some View {
        NavigationStack {
            GeometryReader { geometry in
                ComposerTextInput(text: $text, selection: $selection, focused: $focused, placeholder: L10n.tr("编辑消息"), maximumHeight: geometry.size.height, fillsAvailableHeight: true, identifier: "expanded-input")
                    .padding(.horizontal, 20)
            }.background(Palette.canvas)
                .navigationTitle(L10n.tr("编辑消息")).navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) { Button(L10n.tr("收起"), systemImage: "arrow.down.right.and.arrow.up.left", action: collapse).accessibilityIdentifier("collapse-input") }
                    ToolbarItem(placement: .topBarTrailing) { Button(L10n.tr("发送"), systemImage: "arrow.up", action: send).bold().disabled(!canSend).accessibilityIdentifier("send-expanded-input") }
                }
        }.interactiveDismissDisabled()
    }
}

// Resolve exclusions in the composer's coordinates, including disabled controls.
// The gesture lives behind the content so UITextView keeps native selection gestures.
private struct ComposerControlBounds: PreferenceKey {
    static var defaultValue: [Anchor<CGRect>] = []
    static func reduce(value: inout [Anchor<CGRect>], nextValue: () -> [Anchor<CGRect>]) {
        value += nextValue()
    }
}

extension View {
    func composerControl() -> some View {
        anchorPreference(key: ComposerControlBounds.self, value: .bounds) { [$0] }
    }

    func composerWhitespaceFocus(enabled: Bool, focus: @escaping () -> Void) -> some View {
        backgroundPreferenceValue(ComposerControlBounds.self) { controls in
            GeometryReader { geometry in
                Color.clear
                    .contentShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
                    .onTapGesture { point in
                        guard enabled, !controls.contains(where: { geometry[$0].contains(point) }) else { return }
                        focus()
                    }
                    .accessibilityHidden(true)
            }
        }
    }
}
