import SwiftUI
import UIKit

/// The app icon's potato and sprout as vectors, so they can move and follow dark mode.
/// Geometry is copied from Assets/generate-app-icon.py (1024 pt icon space).
private enum PotatoGeometry {
    static let body = SVGPath.parse("M859.4 553.7C863.3 581.6 851.4 615.1 834.8 642.8C818.2 670.5 788.6 696.9 759.8 720.0C730.9 743.1 697.8 763.9 661.7 781.4C625.5 798.9 584.7 815.5 542.8 824.9C501.0 834.4 453.1 839.9 410.5 838.3C367.9 836.6 322.7 828.3 287.4 814.8C252.0 801.3 218.9 780.3 198.6 757.1C178.3 734.0 167.3 703.9 165.6 676.0C163.8 648.2 174.4 617.7 188.3 590.1C202.2 562.5 223.1 535.2 248.9 510.7C274.7 486.2 306.6 461.0 343.2 443.0C379.7 424.9 425.7 409.8 468.4 402.6C511.0 395.4 557.7 396.0 598.9 399.8C640.1 403.6 680.2 412.7 715.7 425.3C751.2 438.0 788.1 454.1 812.0 475.5C836.0 496.9 855.6 525.8 859.4 553.7Z")
    static let eyes: [(CGPoint, CGFloat)] = [(CGPoint(x: 352, y: 648), 1), (CGPoint(x: 561, y: 694), 0.85), (CGPoint(x: 677, y: 560), 0.75)]
    static let sproutBase = CGPoint(x: 572, y: 408), sproutScale: CGFloat = 1.1
    /// Square box around potato and sprout.
    static let bounds = CGRect(x: 160, y: 170, width: 710, height: 710)
    static let base = CGPoint(x: 516, y: 830)
}

/// The sprout in its own space: the stem starts at the origin and grows up.
private enum SproutGeometry {
    static let stem = SVGPath.parse("M0 0C-2 -34 8 -66 30 -92")
    static let bigLeaf = SVGPath.parse("M26 -86C44 -148 104 -178 170 -168C158 -104 100 -70 26 -86Z")
    static let smallLeaf = SVGPath.parse("M8 -46C-18 -92 -70 -108 -116 -92C-96 -46 -44 -30 8 -46Z")
    static let rib = SVGPath.parse("M40 -92C74 -114 110 -134 146 -154")
    /// Square box around the sprout with its origin near the bottom centre.
    static let bounds = CGRect(x: -128, y: -218, width: 310, height: 310)
}

enum BrandColor {
    static func hex(_ value: UInt32) -> Color {
        Color(red: Double(value >> 16 & 255) / 255, green: Double(value >> 8 & 255) / 255, blue: Double(value & 255) / 255)
    }
    static let potatoLight = hex(0xE6AE72), potatoBase = hex(0xC98545), potatoShade = hex(0xB06C35), eye = hex(0x955326)
    static let leafA = hex(0x79B060), leafB = hex(0x5E9A4B), rib = hex(0xA6D38A)
}

extension GraphicsContext {
    func drawSprout() {
        stroke(SproutGeometry.stem, with: .color(BrandColor.leafB), style: StrokeStyle(lineWidth: 22, lineCap: .round))
        fill(SproutGeometry.bigLeaf, with: .color(BrandColor.leafA))
        stroke(SproutGeometry.rib, with: .color(BrandColor.rib.opacity(0.7)), style: StrokeStyle(lineWidth: 7, lineCap: .round))
        fill(SproutGeometry.smallLeaf, with: .color(BrandColor.leafB))
    }

    /// Draws the potato in icon space; `scale` converts icon points to screen points for the blur.
    func drawPotato(scale: CGFloat) {
        let body = PotatoGeometry.body
        var inside = self
        inside.clip(to: body)
        // A shifted copy leaves a darker crescent along the lower edge.
        inside.fill(Path(CGRect(x: 0, y: 0, width: 1024, height: 1024)), with: .color(BrandColor.potatoShade))
        inside.fill(body.offsetBy(dx: -16, dy: -26), with: .linearGradient(Gradient(colors: [BrandColor.potatoLight, BrandColor.potatoBase]), startPoint: CGPoint(x: 236, y: 466), endPoint: CGPoint(x: 744, y: 782)))
        var glow = inside
        glow.addFilter(.blur(radius: 26 * scale))
        glow.translateBy(x: 346, y: 535); glow.rotate(by: .degrees(-22))
        glow.fill(Path(ellipseIn: CGRect(x: -92, y: -40, width: 184, height: 80)), with: .color(.white.opacity(0.28)))
        for (center, size) in PotatoGeometry.eyes {
            var eye = self
            eye.translateBy(x: center.x, y: center.y); eye.rotate(by: .degrees(-13))
            eye.fill(Path(ellipseIn: CGRect(x: -15 * size, y: -9 * size, width: 30 * size, height: 18 * size)), with: .color(BrandColor.eye))
        }
        var sprout = self
        sprout.translateBy(x: PotatoGeometry.sproutBase.x, y: PotatoGeometry.sproutBase.y)
        sprout.scaleBy(x: PotatoGeometry.sproutScale, y: PotatoGeometry.sproutScale)
        sprout.drawSprout()
    }
}

/// The potato from the app icon, optionally rocking on its base.
struct PotatoMark: View {
    var size: CGFloat
    var tilt: Double = 0
    var body: some View {
        Canvas { context, canvas in
            let box = PotatoGeometry.bounds, scale = canvas.width / box.width
            context.scaleBy(x: scale, y: scale)
            context.translateBy(x: -box.minX, y: -box.minY)
            let base = PotatoGeometry.base
            context.translateBy(x: base.x, y: base.y); context.rotate(by: .degrees(tilt)); context.translateBy(x: -base.x, y: -base.y)
            context.drawPotato(scale: scale)
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

/// The welcome potato; a tap makes it wobble.
struct WelcomePotato: View {
    var haptics = true
    @State private var wobble = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        PotatoMark(size: 76)
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
