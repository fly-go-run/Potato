# Source for the app icon ("探头·夜": the potato rises like a moon into a night sky).
# `python3 generate-app-icon.py` writes AppIcon.svg; export it at 1024 px to
# Assets.xcassets/AppIcon.appiconset/AppIcon.png (the dark variant uses the same art,
# the tinted variant is its grayscale). BrandArt.swift copies the mascot and sprout geometry.
import math

def smooth(pts):
    n = len(pts); d = f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        d += f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}"
    return d + "Z"

def blob(cx, cy, rx, ry, wobble):
    pts = []
    for i in range(18):
        t = 2 * math.pi * i / 18
        r = 1 + sum(a * math.cos(k * t + ph) for k, a, ph in wobble)
        pts.append((cx + rx * r * math.cos(t), cy + ry * r * math.sin(t)))
    return smooth(pts)

def star(cx, cy, r, fill, opacity):
    k = r * .12
    return (f'<path d="M{cx} {cy-r}Q{cx+k} {cy-k} {cx+r} {cy}Q{cx+k} {cy+k} {cx} {cy+r}'
            f'Q{cx-k} {cy+k} {cx-r} {cy}Q{cx-k} {cy-k} {cx} {cy-r}Z" fill="{fill}" opacity="{opacity}"/>')

LEAF_R = "M0 0C20 -70 86 -112 166 -108C150 -34 80 6 0 0Z"
LEAF_L = "M0 0C-18 -58 -72 -90 -136 -84C-122 -26 -62 6 0 0Z"

def sprout(x, y, s):
    tx, ty = x + 20 * s, y - 78 * s
    return (f'<path d="M{x} {y}C{x-2*s:.1f} {y-30*s:.1f} {x+4*s:.1f} {y-56*s:.1f} {tx:.1f} {ty:.1f}" stroke="#4E9C4F" stroke-width="{22*s:.1f}" stroke-linecap="round" fill="none"/>'
            f'<g transform="translate({tx:.1f} {ty+4*s:.1f}) rotate(-14) scale({.78*s:.3f})"><path d="{LEAF_R}" fill="#6CBF67"/></g>'
            f'<g transform="translate({tx-6*s:.1f} {ty+16*s:.1f}) rotate(4) scale({.72*s:.3f})"><path d="{LEAF_L}" fill="#56AB57"/></g>')

def eyes(cx, cy, s, gap):
    o = ""
    for side in (-1, 1):
        x = cx + side * gap * s
        o += (f'<rect x="{x-15*s:.1f}" y="{cy-32*s:.1f}" width="{30*s:.1f}" height="{64*s:.1f}" rx="{15*s:.1f}" fill="url(#eye)"/>'
              f'<rect x="{x-5*s:.1f}" y="{cy-23*s:.1f}" width="{9*s:.1f}" height="{19*s:.1f}" rx="{4.5*s:.1f}" fill="#BFF3FF" opacity=".9"/>')
    return o

CX, CY, RX, RY = 512, 934, 410, 336
body = blob(CX, CY, RX, RY, [(2, .02, 1.5), (3, .015, 1.6)])
hx, hy = CX - RX * .36, CY - RY * .62
art = (
    '<rect width="1024" height="1024" fill="url(#bg)"/>'
    '<circle cx="512" cy="840" r="540" fill="url(#aura)"/>'
    + star(772, 292, 48, "#FFE7A8", .95) + star(850, 404, 22, "#FFFFFF", .8) + star(250, 360, 18, "#C9C3FF", .8)
    + f'<path d="{body}" fill="#DFA266"/>'
    f'<g clip-path="url(#pc)"><path d="{body}" transform="translate(-18 -30)" fill="url(#pf)"/>'
    f'<ellipse cx="{hx:.0f}" cy="{hy:.0f}" rx="{RX*.36:.0f}" ry="{RY*.2:.0f}" transform="rotate(-24 {hx:.0f} {hy:.0f})" fill="#FFF1DA" opacity=".55" filter="url(#soft)"/>'
    f'<path d="{body}" fill="none" stroke="url(#rim)" stroke-width="26" filter="url(#glow)"/></g>'
    f'<circle cx="{CX-RX*.6:.0f}" cy="{CY-RY*.17:.0f}" r="9" fill="#B97638" opacity=".5"/>'
    f'<circle cx="{CX+RX*.6:.0f}" cy="{CY-RY*.4:.0f}" r="8" fill="#B97638" opacity=".5"/>'
    + eyes(512, 752, 1.2, 66) + sprout(508, 604, 1.15)
)
defs = (
    '<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#262B4A"/><stop offset="1" stop-color="#12152A"/></linearGradient>'
    '<radialGradient id="aura"><stop offset=".3" stop-color="#FFB27A" stop-opacity=".4"/><stop offset=".65" stop-color="#7F8CFF" stop-opacity=".3"/><stop offset="1" stop-color="#7F8CFF" stop-opacity="0"/></radialGradient>'
    f'<clipPath id="pc"><path d="{body}"/></clipPath>'
    '<linearGradient id="pf" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F6C98E"/><stop offset="1" stop-color="#E8AE6E"/></linearGradient>'
    '<linearGradient id="eye" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#2E3A5C"/><stop offset="1" stop-color="#1B2238"/></linearGradient>'
    '<linearGradient id="rim" x1="1" y1="0" x2="0" y2="1"><stop stop-color="#B9C2FF" stop-opacity=".9"/><stop offset=".45" stop-color="#B9C2FF" stop-opacity="0"/></linearGradient>'
    '<filter id="soft" x="-80%" y="-80%" width="260%" height="260%"><feGaussianBlur stdDeviation="26"/></filter>'
    '<filter id="glow" x="-80%" y="-80%" width="260%" height="260%"><feGaussianBlur stdDeviation="14"/></filter>'
)
with open("AppIcon.svg", "w") as f:
    f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><defs>{defs}</defs>{art}</svg>')

if __name__ == "__main__":
    # Geometry used by BrandArt.swift for the in-app mascot (full body, not cropped).
    print("mascot body:", blob(512, 610, 300, 252, [(2, .028, 1.5), (3, .018, 1.6), (4, .012, .3)]))
