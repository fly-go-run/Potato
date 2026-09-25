import SwiftUI
import UIKit

/// The app icon's mascot and sprout as vectors, so they can move and follow dark mode.
/// Geometry is copied from Assets/generate-app-icon.py (1024 pt icon space).
private enum MascotGeometry {
    static let body = SVGPath.parse("M815.9 610.0C812.4 638.8 798.8 667.8 784.4 693.3C770.0 718.8 750.4 741.7 729.3 763.2C708.2 784.7 685.3 805.8 657.9 822.3C630.5 838.8 598.2 854.8 564.9 862.0C531.6 869.2 492.5 870.6 458.4 865.5C424.2 860.4 389.4 846.8 359.9 831.3C330.5 815.7 303.7 794.8 281.6 772.4C259.5 750.0 239.7 724.1 227.4 697.0C215.1 670.0 207.3 638.8 207.8 610.0C208.4 581.2 217.4 550.3 230.7 524.0C244.0 497.6 265.1 472.9 287.7 451.9C310.3 430.9 337.4 412.3 366.3 398.0C395.1 383.6 427.8 371.8 460.7 365.8C493.7 359.9 530.1 358.3 564.0 362.1C598.0 365.9 633.3 375.1 664.2 388.5C695.2 401.9 726.2 420.5 749.7 442.5C773.2 464.5 794.2 492.4 805.3 520.3C816.3 548.3 819.4 581.2 815.9 610.0Z")
    /// Top and bottom of the body, for its vertical gradient.
    static let top: CGFloat = 358, bottom: CGFloat = 862
    static let freckles: [(CGPoint, CGFloat, Double)] = [(CGPoint(x: 332, y: 567), 9, 0.5), (CGPoint(x: 692, y: 509), 8, 0.5), (CGPoint(x: 656, y: 761), 7, 0.45)]
    static let eyeCentres = [CGPoint(x: 436.5, y: 600), CGPoint(x: 587.5, y: 600)], eyeScale: CGFloat = 1.18
    static let sproutBase = CGPoint(x: 508, y: 360)
    /// Square box around body and sprout.
    static let bounds = CGRect(x: 163, y: 165, width: 705, height: 705)
    static let base = CGPoint(x: 512, y: 862)
}

/// The sprout in its own space: the stem starts at the origin and grows up.
private enum SproutGeometry {
    static let stem = SVGPath.parse("M0 0C-2 -30 4 -56 20 -78")
    static let bigLeaf = SVGPath.parse("M0 0C20 -70 86 -112 166 -108C150 -34 80 6 0 0Z")
    static let smallLeaf = SVGPath.parse("M0 0C-18 -58 -72 -90 -136 -84C-122 -26 -62 6 0 0Z")
    /// Square box around the sprout with its origin near the bottom centre.
    static let bounds = CGRect(x: -98, y: -215, width: 240, height: 240)
}

enum BrandColor {
    static func hex(_ value: UInt32) -> Color {
        Color(red: Double(value >> 16 & 255) / 255, green: Double(value >> 8 & 255) / 255, blue: Double(value & 255) / 255)
    }
    static let potatoLight = hex(0xE6AE72), potatoBase = hex(0xC98545)
    static let skin = hex(0xDFA266), skinTop = hex(0xF6C98E), skinBottom = hex(0xE8AE6E), freckle = hex(0xB97638)
    static let eyeTop = hex(0x2E3A5C), eyeBottom = hex(0x1B2238), eyeGlint = hex(0xBFF3FF)
    static let stem = hex(0x4E9C4F), leafA = hex(0x6CBF67), leafB = hex(0x56AB57)
}

extension GraphicsContext {
    func drawSprout() {
        stroke(SproutGeometry.stem, with: .color(BrandColor.stem), style: StrokeStyle(lineWidth: 22, lineCap: .round))
        var big = self
        big.translateBy(x: 20, y: -74); big.rotate(by: .degrees(-14)); big.scaleBy(x: 0.78, y: 0.78)
        big.fill(SproutGeometry.bigLeaf, with: .color(BrandColor.leafA))
        var small = self
        small.translateBy(x: 14, y: -62); small.rotate(by: .degrees(4)); small.scaleBy(x: 0.72, y: 0.72)
        small.fill(SproutGeometry.smallLeaf, with: .color(BrandColor.leafB))
    }

    /// Draws the mascot in icon space; `scale` converts icon points to screen points for the blur.
    func drawMascot(scale: CGFloat) {
        let body = MascotGeometry.body
        fill(body, with: .color(BrandColor.skin))
        var inside = self
        inside.clip(to: body)
        inside.fill(body.offsetBy(dx: -18, dy: -30), with: .linearGradient(Gradient(colors: [BrandColor.skinTop, BrandColor.skinBottom]), startPoint: CGPoint(x: 512, y: MascotGeometry.top - 30), endPoint: CGPoint(x: 512, y: MascotGeometry.bottom - 30)))
        var glow = inside
        glow.addFilter(.blur(radius: 72 * scale))
        glow.translateBy(x: 404, y: 454); glow.rotate(by: .degrees(-24))
        glow.fill(Path(ellipseIn: CGRect(x: -108, y: -50, width: 216, height: 100)), with: .color(Color(red: 1, green: 0.945, blue: 0.855).opacity(0.55)))
        for (centre, radius, opacity) in MascotGeometry.freckles {
            fill(Path(ellipseIn: CGRect(x: centre.x - radius, y: centre.y - radius, width: 2 * radius, height: 2 * radius)), with: .color(BrandColor.freckle.opacity(opacity)))
        }
        let s = MascotGeometry.eyeScale
        for centre in MascotGeometry.eyeCentres {
            let eye = Path(roundedRect: CGRect(x: centre.x - 15 * s, y: centre.y - 32 * s, width: 30 * s, height: 64 * s), cornerRadius: 15 * s)
            fill(eye, with: .linearGradient(Gradient(colors: [BrandColor.eyeTop, BrandColor.eyeBottom]), startPoint: CGPoint(x: centre.x, y: centre.y - 32 * s), endPoint: CGPoint(x: centre.x, y: centre.y + 32 * s)))
            fill(Path(roundedRect: CGRect(x: centre.x - 5 * s, y: centre.y - 23 * s, width: 9 * s, height: 19 * s), cornerRadius: 4.5 * s), with: .color(BrandColor.eyeGlint.opacity(0.9)))
        }
        var sprout = self
        sprout.translateBy(x: MascotGeometry.sproutBase.x, y: MascotGeometry.sproutBase.y)
        sprout.drawSprout()
    }
}

/// The mascot from the app icon, optionally rocking on its base.
struct PotatoMark: View {
    var size: CGFloat
    var tilt: Double = 0
    var body: some View {
        Canvas { context, canvas in
            let box = MascotGeometry.bounds, scale = canvas.width / box.width
            context.scaleBy(x: scale, y: scale)
            context.translateBy(x: -box.minX, y: -box.minY)
            let base = MascotGeometry.base
            context.translateBy(x: base.x, y: base.y); context.rotate(by: .degrees(tilt)); context.translateBy(x: -base.x, y: -base.y)
            context.drawMascot(scale: scale)
        }.frame(width: size, height: size).accessibilityHidden(true)
    }
}

/// The sprout alone, swaying around the bottom of its stem.
struct SproutMark: View {
    var size: CGFloat
    var sway: Double = 0
    var body: some View {
        Canvas { context, canvas in
            let box = SproutGeometry.bounds, scale = canvas.width / box.width
            context.scaleBy(x: scale, y: scale)
            context.translateBy(x: -box.minX, y: -box.minY)
            context.rotate(by: .degrees(sway))
            context.drawSprout()
        }.frame(width: size, height: size).accessibilityHidden(true)
    }
}

/// Shown while a reply is on its way: a sprout sways gently. Still with Reduce Motion.
struct ReplyPendingDot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        TimelineView(.animation(paused: reduceMotion)) { context in
            let t = context.date.timeIntervalSinceReferenceDate
            SproutMark(size: 22, sway: reduceMotion ? 0 : 11 * sin(t * .pi / 0.8))
        }.frame(width: 24, height: 24, alignment: .leading)
    }
}

/// The welcome mascot; a tap makes it wobble.
struct WelcomePotato: View {
    var haptics = true
    @State private var wobble = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        PotatoMark(size: 84)
            .keyframeAnimator(initialValue: 0.0, trigger: wobble) { mark, angle in
                mark.rotationEffect(.degrees(angle), anchor: UnitPoint(x: 0.5, y: 0.93))
            } keyframes: { _ in
                KeyframeTrack {
                    SpringKeyframe(-12, duration: 0.14)
                    SpringKeyframe(9, duration: 0.16)
                    SpringKeyframe(-4, duration: 0.16)
                    SpringKeyframe(0, duration: 0.3)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { if !reduceMotion { wobble += 1; if haptics { UIImpactFeedbackGenerator(style: .soft).impactOccurred() } } }
            .accessibilityHidden(true)
    }
}

/// Empty-state drawings in the icon's flat style: the object leads and the sprout ties them together.
struct BrandIllustration: View {
    enum Scene { case papers, chat, computer, search }
    let scene: Scene
    @Environment(\.colorScheme) private var scheme

    private struct Ink {
        let paper, back, edge, line, screen, body, prompt: Color
        let shadow: Double
        init(dark: Bool) {
            let hex = BrandColor.hex
            paper = dark ? hex(0x302A25) : .white
            back = dark ? hex(0x26211D) : hex(0xF1E6D8)
            edge = dark ? hex(0x3A332C) : hex(0xE6D8C6)
            line = dark ? hex(0x4A4038) : hex(0xE4D5C2)
            screen = dark ? hex(0x2B2621) : hex(0xF4EBDF)
            body = dark ? hex(0x3A322B) : hex(0xE4D6C5)
            prompt = dark ? BrandColor.potatoLight : BrandColor.potatoBase
            shadow = dark ? 0.35 : 0.07
        }
    }

    var body: some View {
        Canvas { context, canvas in
            let scale = canvas.width / 280
            context.scaleBy(x: scale, y: scale)
            draw(Ink(dark: scheme == .dark), in: context)
        }.frame(width: 168, height: 108).accessibilityHidden(true)
    }

    private func draw(_ ink: Ink, in context: GraphicsContext) {
        func ground(_ x: CGFloat, _ width: CGFloat) {
            context.fill(Path(ellipseIn: CGRect(x: x - width, y: 153, width: 2 * width, height: 14)), with: .color(.black.opacity(ink.shadow)))
        }
        func sprout(_ x: CGFloat, _ y: CGFloat, scale: CGFloat, rotation: Double) {
            var c = context
            c.translateBy(x: x, y: y); c.rotate(by: .degrees(rotation)); c.scaleBy(x: scale, y: scale)
            c.drawSprout()
        }
        func rotated(_ degrees: Double, around pivot: CGPoint) -> GraphicsContext {
            var c = context
            c.translateBy(x: pivot.x, y: pivot.y); c.rotate(by: .degrees(degrees)); c.translateBy(x: -pivot.x, y: -pivot.y)
            return c
        }
        let edge = StrokeStyle(lineWidth: 2, lineJoin: .round), bar = StrokeStyle(lineWidth: 7, lineCap: .round, lineJoin: .round)
        switch scene {
        case .papers:
            ground(140, 70)
            rotated(8, around: CGPoint(x: 148, y: 94)).fill(Path(roundedRect: CGRect(x: 104, y: 38, width: 88, height: 112), cornerRadius: 12), with: .color(ink.back))
            sprout(150, 58, scale: 0.26, rotation: 6)
            let front = rotated(-4, around: CGPoint(x: 132, y: 106))
            let sheet = Path(roundedRect: CGRect(x: 88, y: 50, width: 88, height: 112), cornerRadius: 12)
            front.fill(sheet, with: .color(ink.paper)); front.stroke(sheet, with: .color(ink.edge), style: edge)
            front.stroke(SVGPath.parse("M106 82H158M106 100H158M106 118H140"), with: .color(ink.line), style: bar)
        case .chat:
            ground(140, 66)
            sprout(172, 52, scale: 0.26, rotation: 8)
            let bubble = SVGPath.parse("M92 50H188C200 50 210 60 210 72V112C210 124 200 134 188 134H130L106 154V134H92C80 134 70 124 70 112V72C70 60 80 50 92 50Z")
            context.fill(bubble, with: .color(ink.paper)); context.stroke(bubble, with: .color(ink.edge), style: edge)
            for x: CGFloat in [116, 140, 164] { context.fill(Path(ellipseIn: CGRect(x: x - 7, y: 85, width: 14, height: 14)), with: .color(ink.line)) }
        case .computer:
            ground(140, 80)
            sprout(182, 52, scale: 0.24, rotation: 10)
            context.fill(Path(roundedRect: CGRect(x: 78, y: 48, width: 124, height: 86), cornerRadius: 12), with: .color(ink.body))
            context.fill(Path(roundedRect: CGRect(x: 88, y: 58, width: 104, height: 66), cornerRadius: 6), with: .color(ink.screen))
            context.stroke(SVGPath.parse("M104 80L116 90L104 100M124 104H142"), with: .color(ink.prompt), style: bar)
            context.fill(SVGPath.parse("M62 136H218L212 148C211 151 208 152 205 152H75C72 152 69 151 68 148Z"), with: .color(ink.body))
        case .search:
            ground(140, 60)
            context.stroke(SVGPath.parse("M168 118L196 146"), with: .color(ink.body), style: StrokeStyle(lineWidth: 16, lineCap: .round))
            let lens = Path(ellipseIn: CGRect(x: 88, y: 40, width: 92, height: 92))
            context.fill(lens, with: .color(ink.paper)); context.stroke(lens, with: .color(ink.body), style: StrokeStyle(lineWidth: 12))
            sprout(134, 114, scale: 0.24, rotation: 0)
        }
    }
}

/// Minimal parser for absolute SVG path data (M, L, H, V, C, Z), enough for the drawings above.
enum SVGPath {
    static func parse(_ data: String) -> Path {
        var tokens: [String] = []
        var number = ""
        for ch in data {
            if ch.isLetter { if !number.isEmpty { tokens.append(number); number = "" }; tokens.append(String(ch)) }
            else if ch == " " || ch == "," { if !number.isEmpty { tokens.append(number); number = "" } }
            else if ch == "-" && !number.isEmpty { tokens.append(number); number = "-" }
            else { number.append(ch) }
        }
        if !number.isEmpty { tokens.append(number) }
        var path = Path(), index = 0, command = "M", current = CGPoint.zero
        func next() -> CGFloat { defer { index += 1 }; return CGFloat(Double(tokens[index]) ?? 0) }
        while index < tokens.count {
            if tokens[index].first?.isLetter == true { command = tokens[index]; index += 1; if command == "Z" { path.closeSubpath(); continue } }
            switch command {
            case "M": current = CGPoint(x: next(), y: next()); path.move(to: current); command = "L"
            case "L": current = CGPoint(x: next(), y: next()); path.addLine(to: current)
            case "H": current.x = next(); path.addLine(to: current)
            case "V": current.y = next(); path.addLine(to: current)
            case "C":
                let c1 = CGPoint(x: next(), y: next()), c2 = CGPoint(x: next(), y: next())
                current = CGPoint(x: next(), y: next()); path.addCurve(to: current, control1: c1, control2: c2)
            default: index += 1
            }
        }
        return path
    }
}
