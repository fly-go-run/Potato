import SwiftUI

@main
struct PotatoMobileApp: App {
    var body: some Scene {
        WindowGroup {
            WorkspaceView()
                .preferredColorScheme(.light)
                .environment(\.locale, Locale(identifier: "zh_CN"))
                .tint(Palette.ink)
        }
    }
}

enum Palette {
    static let canvas = Color(red: 0.973, green: 0.969, blue: 0.957)
    static let ink = Color(red: 0.12, green: 0.12, blue: 0.115)
    static let secondary = Color(red: 0.46, green: 0.46, blue: 0.44)
    static let muted = Color(red: 0.94, green: 0.937, blue: 0.917)
    static let line = Color.black.opacity(0.085)
}
