import SwiftUI

@main
struct PotatoMobileApp: App {
    var body: some Scene {
        WindowGroup {
            WorkspaceView()
                .environment(\.locale, AppLocalization.shared.locale)
                .onReceive(NotificationCenter.default.publisher(for: NSLocale.currentLocaleDidChangeNotification)) { _ in AppLocalization.shared.refreshSystemLanguage() }
                .tint(Palette.ink)
        }
    }
}

extension AppAppearance {
    var interfaceStyle: UIUserInterfaceStyle {
        switch self { case .light: .light; case .dark: .dark; case .automatic: .unspecified }
    }
}

// Apply the preference to this window so sheets and UIKit editors inherit it too.
// Clearing the override lets automatic mode follow live system appearance changes.
struct WindowAppearance: UIViewRepresentable {
    let mode: AppAppearance
    func makeUIView(context: Context) -> AppearanceView { AppearanceView() }
    func updateUIView(_ view: AppearanceView, context: Context) {
        view.style = mode.interfaceStyle
        view.apply()
    }
    final class AppearanceView: UIView {
        var style: UIUserInterfaceStyle = .unspecified
        override func didMoveToWindow() { super.didMoveToWindow(); apply() }
        func apply() {
            guard let window, window.overrideUserInterfaceStyle != style else { return }
            window.overrideUserInterfaceStyle = style
        }
    }
}

enum Palette {
    private static func adaptive(_ light: UIColor, _ dark: UIColor) -> Color {
        Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? dark : light })
    }
    static let canvas = adaptive(UIColor(red: 0.973, green: 0.969, blue: 0.957, alpha: 1), UIColor(white: 0.075, alpha: 1))
    static let ink = adaptive(UIColor(red: 0.12, green: 0.12, blue: 0.115, alpha: 1), UIColor(white: 0.94, alpha: 1))
    static let secondary = adaptive(UIColor(red: 0.44, green: 0.44, blue: 0.42, alpha: 1), UIColor(white: 0.68, alpha: 1))
    static let muted = adaptive(UIColor(red: 0.94, green: 0.937, blue: 0.917, alpha: 1), UIColor(white: 0.16, alpha: 1))
    static let surface = adaptive(.white, UIColor(white: 0.12, alpha: 1))
    static let grouped = adaptive(UIColor(white: 0.97, alpha: 1), UIColor(white: 0.075, alpha: 1))
    static let onInk = adaptive(.white, UIColor(white: 0.10, alpha: 1))
    static let line = adaptive(.black.withAlphaComponent(0.085), .white.withAlphaComponent(0.14))
    static let glassEdge = adaptive(.white.withAlphaComponent(0.6), .white.withAlphaComponent(0.12))
    static let fileAccent = adaptive(UIColor(red: 0.56, green: 0.32, blue: 0.23, alpha: 1), UIColor(red: 0.91, green: 0.68, blue: 0.52, alpha: 1))
    static let fileBackground = adaptive(UIColor(red: 0.98, green: 0.94, blue: 0.91, alpha: 1), UIColor(red: 0.23, green: 0.17, blue: 0.14, alpha: 1))
}
