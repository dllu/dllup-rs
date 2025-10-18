#!/usr/bin/env python3
"""
Generate an animated SVG of a Risley prism scan (script-free).
- Reproduces the canvas animation with fixed omega = -0.743
- The whole scan is one <path>; animation reveals it by animating stroke-dashoffset
- Duration: 20 s, loops indefinitely
- ViewBox: 600 x 600 (same as your canvas)

Tip: If the output file is too large, reduce STEPS (fewer points).
"""

import math
from pathlib import Path


def risley_points(
    W,
    H,
    r,
    a_ratio,
    dtheta,
    seconds,
    fps,
    seg_per_frame,
    omega,
):
    """
    Return list of (x, y) points for the scan.
    Matches the JS loop:
        for each frame: do seg_per_frame steps of theta += dtheta, emit point
        total frames = seconds * fps
    """
    total_steps = int(seconds * fps * seg_per_frame)
    a = a_ratio * r
    theta = 0.0

    pts = []
    # Optionally add the initial point (before the first increment)
    # We'll match the canvas behavior more closely by starting after first increment.
    length = 0
    lastx = 0
    lasty = 0

    for _ in range(total_steps):
        theta += dtheta
        x = r + a * math.sin(theta) + a * math.sin(omega * theta)
        y = r + a * math.cos(theta) + a * math.cos(omega * theta)

        length += math.sqrt((x - lastx) ** 2 + (y - lasty) ** 2)

        lastx = x
        lasty = y
        pts.append((x, y))
    return pts, length


def build_path_d(points, precision=1):
    if not points:
        return "M0 0"
    fmt = f"{{:.{precision}f}}"
    x0, y0 = points[0]
    parts = [f"M{fmt.format(x0)} {fmt.format(y0)}"]
    # Use 'L' commands; browsers handle very long paths fine
    for x, y in points[1:]:
        parts.append(f"L{fmt.format(x)} {fmt.format(y)}")
    return " ".join(parts)


def generate_svg(
    W=600,
    H=600,
    omega=-0.743,
    seconds=4.0,
    fps=60.0,
    seg_per_frame=100,
    dtheta=0.01,
    a_ratio=0.48,
    stroke="#ff2a2a",
    stroke_width=2,
    precision=1,
):
    pts, length = risley_points(
        W=W,
        H=H,
        r=min(W, H) / 2.0,
        a_ratio=a_ratio,
        dtheta=dtheta,
        seconds=seconds,
        fps=fps,
        seg_per_frame=seg_per_frame,
        omega=omega,
    )
    d = build_path_d(pts, precision=precision)

    # Normalize path length to 1000 units so we can animate dashoffset easily.
    # We start fully hidden (dashoffset=1000) and reveal to 0 over 'seconds'.
    svg = f'''<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     width="{W}" height="{H}" viewBox="0 0 {W} {H}">
  <defs>
    <style>
      .trace {{ fill: none; stroke: {stroke}; stroke-width: {stroke_width};
               vector-effect: non-scaling-stroke; }}
    </style>
  </defs>

  <!-- Background (optional); comment out if you prefer transparent -->
  <!-- <rect x="0" y="0" width="{W}" height="{H}" fill="#ffffff"/> -->

  <path class="trace"
        d="{d}"
        pathLength="{length}"
        stroke-dasharray="{length}"
        stroke-dashoffset="0">
    <animate attributeName="stroke-dashoffset"
             from="{length}" to="0"
             dur="{seconds}s"
             repeatCount="indefinite"/>
  </path>
</svg>
'''
    return svg


def main():
    svg = generate_svg()
    out = Path("risley_omega_-0.743.svg")
    out.write_text(svg, encoding="utf-8")
    print(f"Wrote {out.resolve()}")


if __name__ == "__main__":
    main()
