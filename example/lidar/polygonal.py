#!/usr/bin/env python3
"""
Generate an animated SVG of a red laser being reflected by a rotating hexagonal mirror.
- Physically correct specular reflection per frame (no <script> in the SVG).
- The hex rotates continuously; the active facet switches with flyback jumps.
- Incident beam animates to the exact facet hit-point; reflected beam animates to the viewport edge.
- All <animate> blocks have equal-length keyTimes/values arrays.

Tested output in Chrome and Firefox.
"""

import math
from pathlib import Path


def generate_svg_string(
    N=360,  # number of animation samples per revolution
    dur=10.0,  # seconds per revolution
    W=600,
    H=520,  # SVG size
    cx=500.0,
    cy=301.0,  # mirror center
    R=95.0,  # hexagon circumradius
    Sx=50,
    Sy=255.0,  # laser source
    dx=1.0,
    dy=0.0,  # incident direction (unit)
):
    # --- Geometry helpers ---
    def rot(vx, vy, th):
        c, s = math.cos(th), math.sin(th)
        return (vx * c - vy * s, vx * s + vy * c)

    def cross(ax, ay, bx, by):
        return ax * by - ay * bx

    def dot(ax, ay, bx, by):
        return ax * bx + ay * by

    def norm(ax, ay):
        m = math.hypot(ax, ay)
        return (0.0, 0.0) if m == 0 else (ax / m, ay / m)

    def ray_seg_intersect(Sx, Sy, dx, dy, x1, y1, x2, y2, eps=1e-9):
        # Solve S + t d = p1 + u (p2-p1),  t>=0, 0<=u<=1
        ex, ey = x2 - x1, y2 - y1
        denom = cross(dx, dy, ex, ey)
        if abs(denom) < eps:
            return None
        sxpx, sxpy = x1 - Sx, y1 - Sy
        t = cross(sxpx, sxpy, ex, ey) / denom
        u = cross(sxpx, sxpy, dx, dy) / denom
        if t >= -1e-9 and -1e-9 <= u <= 1 + 1e-9:
            return t, u
        return None

    def outward_normal(x1, y1, x2, y2):
        # Choose the unit normal pointing AWAY from polygon center (cx, cy)
        ex, ey = x2 - x1, y2 - y1
        n1 = norm(ey, -ex)
        n2 = norm(-ey, ex)
        mx, my = (x1 + x2) / 2, (y1 + y2) / 2
        to_center = (cx - mx, cy - my)
        return n1 if dot(n1[0], n1[1], to_center[0], to_center[1]) < 0 else n2

    def clip_to_rect(x, y, rx, ry, W, H, eps=1e-9):
        # Clip ray P(t) = (x,y) + t*(rx,ry), t>0, to the rectangle [0,W]x[0,H]
        ts = []
        if rx > eps:
            ts.append((W - x) / rx)
        elif rx < -eps:
            ts.append((0 - x) / rx)
        if ry > eps:
            ts.append((H - y) / ry)
        elif ry < -eps:
            ts.append((0 - y) / ry)
        ts = [t for t in ts if t > eps]
        if not ts:
            return (x, y)
        tmin = min(ts)
        ex, ey = x + rx * tmin, y + ry * tmin
        # guard tiny numeric drift
        ex = min(max(ex, 0.0), W)
        ey = min(max(ey, 0.0), H)
        return (ex, ey)

    # Base hex in local coords (CCW), one vertex at angle 0°
    verts0 = [
        (R * math.cos(math.radians(60 * k)), R * math.sin(math.radians(60 * k)))
        for k in range(6)
    ]

    # Normalize incident direction
    dlen = math.hypot(dx, dy)
    dx, dy = dx / dlen, dy / dlen

    # --- Sample animation frames ---
    keys, hx, hy, rx2, ry2 = [], [], [], [], []

    for i in range(N):
        tnorm = i / (N - 1)  # 0..1
        theta = 2.0 * math.pi * tnorm  # mirror rotation angle

        # Rotate hex and translate to center
        verts = [rot(x, y, theta) for (x, y) in verts0]
        verts = [(x + cx, y + cy) for (x, y) in verts]

        # Find intersection with the ACTIVE (front-facing) facet
        hits = []
        for k in range(6):
            x1, y1 = verts[k]
            x2, y2 = verts[(k + 1) % 6]
            hit = ray_seg_intersect(Sx, Sy, dx, dy, x1, y1, x2, y2)
            if hit:
                t, u = hit
                px, py = Sx + t * dx, Sy + t * dy
                n = outward_normal(x1, y1, x2, y2)  # outward normal of that edge
                if dot(dx, dy, n[0], n[1]) < 0:  # only front-facing facet reflects
                    hits.append((t, px, py, n))

        if not hits:
            # Extremely unlikely with this geometry, but keep arrays valid
            if i == 0:
                px, py = Sx, Sy
                endx, endy = Sx, Sy
            else:
                px, py = hx[-1], hy[-1]
                endx, endy = rx2[-1], ry2[-1]
        else:
            hits.sort(key=lambda item: item[0])  # nearest hit
            _, px, py, n = hits[0]

            # Specular reflection: r = d - 2*(d·n)*n
            dn = dx * n[0] + dy * n[1]
            rx, ry = dx - 2 * dn * n[0], dy - 2 * dn * n[1]

            # Clip reflected ray to the viewport boundary
            endx, endy = clip_to_rect(px, py, rx, ry, W, H)

        keys.append(tnorm)
        hx.append(px)
        hy.append(py)
        rx2.append(endx)
        ry2.append(endy)

    # --- Formatting for SMIL <animate> ---
    def fmt_vals(vals):
        out = []
        for v in vals:
            if abs(v - round(v)) < 1e-6:
                out.append(f"{round(v):.0f}")
            else:
                out.append(f"{v:.2f}")
        return ";".join(out)

    def fmt_keys(keys):
        return ";".join(f"{k:.6f}" for k in keys)

    pts = " ".join(f"{x:.2f},{y:.2f}" for (x, y) in verts0)

    svg = f'''<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
  <style>
    .hex {{ fill:none; stroke:#8a8f98; stroke-width:2; vector-effect:non-scaling-stroke }}
    .beam {{ stroke:#ff2a2a; stroke-width:4; stroke-linecap:round; vector-effect:non-scaling-stroke }}
    .out  {{ stroke-width:4 }}
  </style>

  <!-- Rotating hex mirror at ({cx}, {cy}) -->
  <g transform="translate({cx},{cy})">
    <polygon class="hex" points="{pts}">
      <animateTransform attributeName="transform" type="rotate"
        from="0" to="360" dur="{dur}s" repeatCount="indefinite"/>
    </polygon>
  </g>

  <!-- Incident beam: source -> hit point -->
  <line class="beam" x1="{Sx}" y1="{Sy}" x2="{hx[0]:.2f}" y2="{hy[0]:.2f}">
    <animate attributeName="x2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(hx)}"/>
    <animate attributeName="y2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(hy)}"/>
  </line>

  <!-- Reflected beam: hit point -> clipped viewport edge -->
  <line class="beam out" x1="{hx[0]:.2f}" y1="{hy[0]:.2f}" x2="{rx2[0]:.2f}" y2="{ry2[0]:.2f}">
    <animate attributeName="x1" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(hx)}"/>
    <animate attributeName="y1" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(hy)}"/>
    <animate attributeName="x2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(rx2)}"/>
    <animate attributeName="y2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}"
      values="{fmt_vals(ry2)}"/>
  </line>

  <!-- Tiny source marker -->
  <circle cx="{Sx}" cy="{Sy}" r="3" fill="#ff2a2a"/>
</svg>
'''
    return svg


def main(out_path="hex_scanner.svg"):
    svg = generate_svg_string(
        N=360,  # adjust for smoothness vs file size
        dur=10.0,  # seconds per revolution
    )
    Path(out_path).write_text(svg, encoding="utf-8")
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()
