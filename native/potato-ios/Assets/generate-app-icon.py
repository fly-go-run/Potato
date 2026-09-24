# Source for the app icon: `python3 generate-app-icon.py` writes AppIcon*.svg, then export each at 1024 px
# into Assets.xcassets/AppIcon.appiconset. BrandArt.swift copies the potato and sprout geometry.
import math, sys
def blob(cx,cy,a,b,rot,perturb,n=14):
    pts=[]
    for i in range(n):
        t=2*math.pi*i/n
        r=1+sum(amp*math.cos(k*t+ph) for k,amp,ph in perturb)
        x=a*r*math.cos(t); y=b*r*math.sin(t)
        c,s=math.cos(rot),math.sin(rot)
        pts.append((cx+x*c-y*s, cy+x*s+y*c))
    # closed Catmull-Rom -> cubic bezier
    d=f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"
    for i in range(n):
        p0,p1,p2,p3=pts[i-1],pts[i],pts[(i+1)%n],pts[(i+2)%n]
        c1=(p1[0]+(p2[0]-p0[0])/6, p1[1]+(p2[1]-p0[1])/6)
        c2=(p2[0]-(p3[0]-p1[0])/6, p2[1]-(p3[1]-p1[1])/6)
        d+=f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}"
    return d+"Z", pts
ROT=math.radians(-10)
PERT=[(2,0.03,0.9),(3,0.022,-0.5),(4,0.012,0.3)]
CX,CY,A,B=506,616,342,216
body,_=blob(CX,CY,A,B,ROT,PERT,16)
def at(u,v):
    # point in potato local coords (u,v in -1..1) -> icon coords
    x=A*u; y=B*v; c,s=math.cos(ROT),math.sin(ROT)
    return CX+x*c-y*s, CY+x*s+y*c
def theme(dark=False, tint=False):
    bg=("#2A2521","#1D1916") if dark else ("#FFFBF4","#F5EADB")
    base,light,shade,eye="#C98545","#E6AE72","#B06C35","#955326"
    leafA,leafB,stem,rib="#79B060","#5E9A4B","#5E9A4B","#A6D38A"
    if tint:
        bg=("#1C1C1C","#141414"); base,light,shade,eye="#CFCFCF","#F2F2F2","#AFAFAF","#8A8A8A"; leafA,leafB,stem,rib="#CFCFCF","#B0B0B0","#B0B0B0","#E8E8E8"
    return locals()
def icon(dark=False,tint=False,mask=True):
    T=theme(dark,tint)
    o=[f'<defs><linearGradient id="bg{dark}{tint}" x1="0" y1="0" x2="0" y2="1"><stop stop-color="{T["bg"][0]}"/><stop offset="1" stop-color="{T["bg"][1]}"/></linearGradient>'
       f'<clipPath id="body{dark}{tint}"><path d="{body}"/></clipPath>'
       f'<radialGradient id="sh{dark}{tint}" cx="50%" cy="50%" r="50%"><stop stop-color="#000" stop-opacity=".16"/><stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient></defs>']
    o.append(f'<rect width="1024" height="1024" fill="url(#bg{dark}{tint})"/>')
    # soft ground shadow
    o.append(f'<ellipse cx="{CX+10}" cy="{CY+205}" rx="270" ry="34" fill="url(#sh{dark}{tint})"/>')
    g0=at(-0.7,-0.9); g1=at(0.6,0.95)
    o.append(f'<defs><linearGradient id="pg{dark}{tint}" gradientUnits="userSpaceOnUse" x1="{g0[0]:.0f}" y1="{g0[1]:.0f}" x2="{g1[0]:.0f}" y2="{g1[1]:.0f}"><stop stop-color="{T["light"]}"/><stop offset="1" stop-color="{T["base"]}"/></linearGradient>'
             f'<filter id="blur{dark}{tint}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="14"/></filter></defs>')
    o.append(f'<g clip-path="url(#body{dark}{tint})"><rect width="1024" height="1024" fill="{T["shade"]}"/>'
             f'<path d="{body}" transform="translate(-16 -26)" fill="url(#pg{dark}{tint})"/>')
    hx,hy=at(-0.42,-0.5)
    o.append(f'<ellipse cx="{hx:.0f}" cy="{hy:.0f}" rx="92" ry="40" transform="rotate(-22 {hx:.0f} {hy:.0f})" fill="#FFFFFF" opacity="{0.10 if tint else 0.28}" filter="url(#blur{dark}{tint})"/></g>')
    # eyes: small tilted ellipses with a light lip
    for (u,v,s) in [(-0.46,0.02,1.0),(0.12,0.40,0.85),(0.52,-0.12,0.75)]:
        x,y=at(u,v)
        o.append(f'<ellipse cx="{x:.0f}" cy="{y:.0f}" rx="{15*s:.0f}" ry="{9*s:.0f}" transform="rotate(-13 {x:.0f} {y:.0f})" fill="{T["eye"]}"/>')
    # sprout from top-right eye region
    bx,by=at(0.30,-0.93)
    o.append(f'<g transform="translate({bx:.0f} {by+8:.0f}) scale(1.1)">'
             f'<path d="M0 0C-2 -34 8 -66 30 -92" stroke="{T["stem"]}" stroke-width="22" stroke-linecap="round" fill="none"/>'
             # big right leaf
             f'<path d="M26 -86C44 -148 104 -178 170 -168C158 -104 100 -70 26 -86Z" fill="{T["leafA"]}"/>'
             f'<path d="M40 -92C74 -114 110 -134 146 -154" stroke="{T["rib"]}" stroke-width="7" stroke-linecap="round" fill="none" opacity=".7"/>'
             # small left leaf
             f'<path d="M8 -46C-18 -92 -70 -108 -116 -92C-96 -46 -44 -30 8 -46Z" fill="{T["leafB"]}"/>'
             '</g>')
    g="".join(o)
    return g
def sheet():
    W,H=1600,1600
    s=[f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}"><rect width="{W}" height="{H}" fill="#EFECE7"/>']
    variants=[("light",False,False),("dark",True,False),("tint",False,True)]
    for i,(n,d,t) in enumerate(variants):
        s.append(f'<svg x="{40+i*520}" y="40" width="480" height="480" viewBox="0 0 1024 1024"><clipPath id="m{n}"><rect width="1024" height="1024" rx="229"/></clipPath><g clip-path="url(#m{n})">{icon(d,t)}</g></svg>')
    for row,wall in enumerate(("#CFDDEA","#15171C")):
        y=580+row*340
        s.append(f'<rect x="40" y="{y}" width="1520" height="300" rx="36" fill="{wall}"/>')
        for j,sz in enumerate((180,120,60,40)):
            x=100+[0,260,460,600][j]
            d= row==1 and j<2 and False
            s.append(f'<svg x="{x}" y="{y+150-sz/2}" width="{sz}" height="{sz}" viewBox="0 0 1024 1024"><clipPath id="k{row}{j}"><rect width="1024" height="1024" rx="229"/></clipPath><g clip-path="url(#k{row}{j})">{icon(dark=(row==1 and j>=0 and False))}</g></svg>')
        # dark home screen with dark icon variant
        if row==1:
            for j,sz in enumerate((180,120,60)):
                x=900+[0,260,460][j]
                s.append(f'<svg x="{x}" y="{y+150-sz/2}" width="{sz}" height="{sz}" viewBox="0 0 1024 1024"><clipPath id="kd{j}"><rect width="1024" height="1024" rx="229"/></clipPath><g clip-path="url(#kd{j})">{icon(dark=True)}</g></svg>')
    s.append('</svg>'); return "".join(s)
open("AppIcon.svg","w").write(f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">{icon()}</svg>')
open("AppIcon-dark.svg","w").write(f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">{icon(True)}</svg>')
open("AppIcon-tinted.svg","w").write(f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">{icon(False,True)}</svg>')
