// Local, disposable UI fixture. No files, network, clipboard or user data.
import AppKit

final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    let input = NSTextField(string: "initial")
    let output = NSTextField(labelWithString: "Clicks: 0")
    var clicks = 0
    var button: NSButton!
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 460, height: 240),
                          styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        window.title = "Cua Native Fixture"
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        input.frame = NSRect(x: 30, y: 145, width: 400, height: 30)
        input.setAccessibilityLabel("Integration input")
        button = NSButton(title: "Increment", target: self, action: #selector(increment))
        button.frame = NSRect(x: 30, y: 90, width: 160, height: 32)
        output.frame = NSRect(x: 220, y: 95, width: 180, height: 25)
        window.contentView!.addSubview(input)
        window.contentView!.addSubview(button)
        window.contentView!.addSubview(output)
        NSApplication.shared.setAccessibilityWindows([window!])
        window.makeKeyAndOrderFront(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
    }
    @objc func increment() { clicks += 1; output.stringValue = "Clicks: \(clicks)"; button.title = "Increment · Clicks: \(clicks)" }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
let app = NSApplication.shared
let delegate = Delegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
