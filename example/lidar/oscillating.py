#!/usr/bin/env python3
"""
Generate an animated SVG of a red laser reflecting from an oscillating flat mirror.
- Physically correct specular reflection per frame (no <script> in the SVG).
- Mirror rotates back & forth (sinusoidal) about its center.
- Incident beam animates to the exact facet hit-point; reflected beam animates to the viewport edge.
- All <animate> blocks have equal-length keyTimes/values arrays.
"""

import math
from pathlib import Path


def generate_svg_string(
    # Animation sampling
    N=360,  # samples per oscillation
    dur=0.9,  # seconds per oscillation
    # Canvas
    W=600,
    H=520,  # SVG size
    # Laser source (ray origin and direction)
    Sx=50.0,
    Sy=350.0,
    dx=1.0,
    dy=0.0,  # unit direction (normalized below)
    # Mirror geometry (center, half-length, thickness)
    cx=400.0,
    cy=350.0,
    L=100.0,  # half-length of the flat mirror (total length = 2L)
    thick=3.0,  # visual thickness of the mirror
    # Mirror oscillation (degrees)
    theta0_deg=-50.0,  # base angle (deg) relative to +x axis
    amp_deg=35.0,  # ± amplitude (deg) for back-and-forth swing
):
    # --- Helpers ---
    def cross(ax, ay, bx, by):
        return ax * by - ay * bx

    def dot(ax, ay, bx, by):
        return ax * bx + ay * by

    def norm(ax, ay):
        m = math.hypot(ax, ay)
        return (ax / m, ay / m) if m != 0 else (0.0, 0.0)

    def clip_to_rect(x, y, rx, ry, W, H, eps=1e-9):
        """Clip ray P(t)=(x,y)+t*(rx,ry), t>0 to rectangle [0,W]×[0,H]; return first boundary hit."""
        ts = []
        if abs(rx) > eps:
            t = (W - x) / rx
            if t > eps:
                ts.append(t)
            t = (0 - x) / rx
            if t > eps:
                ts.append(t)
        if abs(ry) > eps:
            t = (H - y) / ry
            if t > eps:
                ts.append(t)
            t = (0 - y) / ry
            if t > eps:
                ts.append(t)
        if not ts:
            return (x, y)
        tmin = min(ts)
        ex, ey = x + rx * tmin, y + ry * tmin
        # clamp tiny drift
        ex = min(max(ex, 0.0), W)
        ey = min(max(ey, 0.0), H)
        return (ex, ey)

    # Normalize incident direction
    dlen = math.hypot(dx, dy)
    dx, dy = dx / dlen, dy / dlen

    # Precompute samples
    keys = []
    # beam endpoints
    hx, hy = [], []  # hit point on mirror
    rx2, ry2 = [], []  # reflected beam clipped endpoint
    # mirror rotation values (deg)
    rot_deg = []

    for i in range(N):
        tnorm = i / (N - 1)  # 0..1
        theta_deg = theta0_deg + amp_deg * math.sin(2 * math.pi * tnorm)
        theta = math.radians(theta_deg)
        rot_deg.append(theta_deg)
        keys.append(tnorm)

        # Mirror local basis (tangent along mirror, normal perpendicular)
        ux, uy = math.cos(theta), math.sin(theta)  # unit tangent
        # choose normal so that d·n < 0 (incoming side)
        n0x, n0y = -uy, ux  # one perpendicular
        dn = dot(dx, dy, n0x, n0y)
        if dn < 0:
            nx, ny = n0x, n0y
        else:
            nx, ny = -n0x, -n0y
            dn = -dn

        # Ray–mirror intersection: S + t*d = M + s*u, |s| <= L
        # cross((S - M) + t d, u) = 0  -> t = cross(M - S, u) / cross(d, u)
        denom = cross(dx, dy, ux, uy)
        # With chosen angles, denom stays nonzero; still guard:
        if abs(denom) < 1e-10:
            # fall back to previous valid point if any
            if i == 0:
                px, py = Sx, Sy
                ex, ey = px, py
            else:
                px, py = hx[-1], hy[-1]
                ex, ey = rx2[-1], ry2[-1]
        else:
            t = cross(cx - Sx, cy - Sy, ux, uy) / denom
            px, py = (Sx + t * dx, Sy + t * dy)
            # Position along mirror
            s = dot(px - cx, py - cy, ux, uy)
            if abs(s) > L:
                # Beam misses finite mirror; clamp to end cap (visual continuity)
                s_clamped = max(min(s, L), -L)
                px, py = (cx + s_clamped * ux, cy + s_clamped * uy)

            # Reflect: r = d - 2*(d·n)*n
            rx, ry = dx - 2 * dn * nx, dy - 2 * dn * ny
            ex, ey = clip_to_rect(px, py, rx, ry, W, H)

        hx.append(px)
        hy.append(py)
        rx2.append(ex)
        ry2.append(ey)

    # --- Formatting for SMIL ---
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

    # SVG mirror shape: rectangle centered at origin, rotated inside a translated group
    rect_x = -L
    rect_y = -thick / 2
    rect_w = 2 * L
    rect_h = thick

    svg = f'''<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
  <style>
    .mirror {{ fill:#ddd; stroke:#8a8f98; stroke-width:2; vector-effect:non-scaling-stroke }}
    .beam   {{ stroke:#ff2a2a; stroke-width:4; stroke-linecap:round; vector-effect:non-scaling-stroke }}
    .out    {{ stroke-width:4 }}
  </style>

  <!-- Incident beam: source -> hit point -->
  <line class="beam" x1="{Sx:.2f}" y1="{Sy:.2f}" x2="{hx[0]:.2f}" y2="{hy[0]:.2f}">
    <animate attributeName="x2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(hx)}"/>
    <animate attributeName="y2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(hy)}"/>
  </line>

  <!-- Reflected beam: hit point -> clipped viewport edge -->
  <line class="beam out" x1="{hx[0]:.2f}" y1="{hy[0]:.2f}" x2="{rx2[0]:.2f}" y2="{ry2[0]:.2f}">
    <animate attributeName="x1" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(hx)}"/>
    <animate attributeName="y1" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(hy)}"/>
    <animate attributeName="x2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(rx2)}"/>
    <animate attributeName="y2" dur="{dur}s" repeatCount="indefinite" calcMode="linear"
      keyTimes="{fmt_keys(keys)}" values="{fmt_vals(ry2)}"/>
  </line>

  <!-- Oscillating flat mirror at ({cx}, {cy}) -->
  <g transform="translate({cx},{cy})">
    <rect class="mirror" x="{rect_x:.2f}" y="{rect_y:.2f}" width="{rect_w:.2f}" height="{rect_h:.2f}" rx="{rect_h / 2:.2f}" ry="{rect_h / 2:.2f}">
      <animateTransform attributeName="transform" type="rotate"
        dur="{dur}s" repeatCount="indefinite" calcMode="linear"
        keyTimes="{fmt_keys(keys)}"
        values="{fmt_vals(rot_deg)}"/>
    </rect>
  </g>

  <!-- Tiny source marker -->
  <circle cx="{Sx:.2f}" cy="{Sy:.2f}" r="3" fill="#ff2a2a"/>
</svg>
'''
    return svg


def main(out_path="flat_mirror_scanner.svg"):
    svg = generate_svg_string(
        N=360,  # smoothness
        dur=10.0,  # seconds per oscillation
        # You can tweak geometry/angles here; defaults are chosen so the beam always hits.
        # theta0_deg=40.0, amp_deg=20.0, L=200 keep intersection on-segment for the chosen S and M.
    )
    Path(out_path).write_text(svg, encoding="utf-8")
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()
