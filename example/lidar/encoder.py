#!/usr/bin/env python3
"""
Offset encoder ring diagram -> SVG

- Left: ring with true center (+) and offset center (+), plus "true" and
  "measured" direction rays intersecting the ring rim.
- Right (top): theta vs time for true (magenta) and measured (green).
- Right (bottom): delta-theta vs time (blue).

MODEL:
    theta_true(t) = omega * t                # parameterized true angle
    theta_meas(t) = theta_true(t) + A*sin(theta_true(t) + phase)

Tweak the PARAMETERS section to change amplitude/phase/omega or the geometry.
"""

from pathlib import Path
import math
import numpy as np

# -------------------- PARAMETERS --------------------
W, H = 1100, 720  # SVG size

# Encoder geometry (left panel)
ring_center = (300.0, 320.0)  # "true center" on canvas
r_outer = 250.0
r_inner = 230.0
n_ticks = 100
# Offset center (physical misalignment)
offset_vec = (30.0, 0.0)  # dx, dy from true center

# Directions (choose a specific ray angle to show)
ray_theta = math.radians(53.0)  # radians for the magenta "true" direction

# Time / angle model
omega = 2 * math.pi / 6.0  # rad/s (1 rev in 6 s)
A = math.radians(12.0)  # error amplitude in radians
phase = math.radians(20.0)  # phase of sinusoidal error
T = 6.0  # seconds shown (1 rev)

# Plot boxes (right side)
plot_pad = 0
plot_w = 380
plot_h = 230
plot1_origin = (640, 120)  # theta vs t
plot2_origin = (640, 430)  # delta-theta vs t

# Sampling
N = 360
ts = np.linspace(0.0, T, N)
theta_true = omega * ts
theta_meas = np.atan2(
    np.sin(theta_true) + offset_vec[1] / r_outer,
    np.cos(theta_true) + offset_vec[0] / r_outer,
)
theta_meas[theta_meas < 0] += np.pi * 2
dtheta = theta_meas - theta_true


# -------------------- HELPERS --------------------
def poly(points):
    return " ".join(f"{x:.2f},{y:.2f}" for x, y in points)


def line(x1, y1, x2, y2):
    return f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"/>'


def path_from_xy(xs, ys):
    return "M " + " L ".join(f"{x:.2f},{y:.2f}" for x, y in zip(xs, ys))


def circle(x, y, r, cls=""):
    c = f' class="{cls}"' if cls else ""
    return f'<circle{c} cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}"/>'


def plus(x, y, s=8, cls=""):
    c = f' class="{cls}"' if cls else ""
    return f"<g{c}>" + line(x - s, y, x + s, y) + line(x, y - s, x, y + s) + "</g>"


def text(x, y, s, cls="", anchor="start"):
    c = f' class="{cls}"' if cls else ""
    return f'<text{c} x="{x:.2f}" y="{y:.2f}" text-anchor="{anchor}">{s}</text>'


# -------------------- LEFT PANEL: RING --------------------
cx, cy = ring_center
ox, oy = cx + offset_vec[0], cy + offset_vec[1]

# Tick angles (irregular-ish spacing optional; keep uniform here)
tick_angles = np.linspace(0, 2 * math.pi, n_ticks, endpoint=False)

ring_svg = []
# Outer & inner circles
ring_svg.append(circle(cx, cy, r_outer, "ring"))
ring_svg.append(circle(cx, cy, r_inner, "ring inner"))

# Ticks
for a in tick_angles:
    x0 = cx + r_inner * math.cos(a)
    y0 = cy + r_inner * math.sin(a)
    x1 = cx + r_outer * math.cos(a)
    y1 = cy + r_outer * math.sin(a)
    ring_svg.append(
        f'<line class="tick" x1="{x0:.2f}" y1="{y0:.2f}" x2="{x1:.2f}" y2="{y1:.2f}"/>'
    )

# True direction ray (magenta) passes through the rim
rim_x = cx + r_outer * math.cos(ray_theta)
rim_y = cy + r_outer * math.sin(ray_theta)
ring_svg.append(
    f'<line class="true" x1="{cx:.2f}" y1="{cy:.2f}" x2="{rim_x:.2f}" y2="{rim_y:.2f}"/>'
)

# Measured direction is ray from offset center intersecting same rim point
ring_svg.append(
    f'<line class="measured" x1="{ox:.2f}" y1="{oy:.2f}" x2="{rim_x:.2f}" y2="{rim_y:.2f}"/>'
)

# Centers
ring_svg.append(plus(cx, cy, 9, "cross"))
ring_svg.append(plus(ox, oy, 9, "cross green"))

ring_svg.append(text(cx - 88, cy + 78, "encoder", "label"))
ring_svg.append(text(cx - 88, cy + 78 + 24, "center", "label"))
ring_svg.append(text(ox + 14, oy - 60, "rotation", "label green"))
ring_svg.append(text(ox + 14, oy - 60 + 24, "center", "label green"))

ring_svg.append(text(720, 70, "true direction", "label label-true"))
ring_svg.append(text(720, 100, "measured direction", "label label-measured"))


# -------------------- RIGHT SIDE: PLOTS --------------------
def axes(x0, y0, w, h, y_arrow=True, x_arrow=True):
    out = []
    out.append(line(x0, y0, x0, y0 + h))  # y axis
    out.append(line(x0, y0 + h, x0 + w, y0 + h))  # x axis
    if y_arrow:
        out.append(
            f'<path d="M {x0 - 6:.1f},{y0 + 10:.1f} L {x0:.1f},{y0:.1f} L {x0 + 6:.1f},{y0 + 10:.1f}" class="axis"/>'
        )
    if x_arrow:
        out.append(
            f'<path d="M {x0 + w - 10:.1f},{y0 + h - 6:.1f} L {x0 + w:.1f},{y0 + h:.1f} L {x0 + w - 10:.1f},{y0 + h + 6:.1f}" class="axis"/>'
        )
    return out


plot_svg = []

# Top plot: theta vs t
x0, y0 = plot1_origin
w, h = plot_w, plot_h
plot_svg += axes(x0, y0, w, h)
# y max ticks dotted at 2π
# plot_svg.append(f'<path class="dotted" d="M {x0:.1f},{y0 + 10:.1f} H {x0 + w:.1f}"/>')
plot_svg.append(text(x0 - 22, y0 + 20, "θ", "label", "end"))
plot_svg.append(text(x0 + w + 18, y0 + h + 4, "t", "label"))

# Map theta to y (0 -> bottom, 2π -> top-ish)
t_norm = (ts - ts.min()) / (ts.max() - ts.min())
x_vals = x0 + plot_pad + (w - 2 * plot_pad) * t_norm


def y_from_theta(th):
    th0 = 0.0
    th1 = 2 * math.pi
    # th_clip = np.clip(th, th0, th1)
    y = y0 + h - plot_pad - (h - 2 * plot_pad) * (th - th0) / (th1 - th0)
    return y


yt_true = y_from_theta(theta_true)
yt_meas = y_from_theta(theta_meas)
plot_svg.append(f'<path class="true" d="{path_from_xy(x_vals, yt_true)}"/>')
plot_svg.append(f'<path class="measured" d="{path_from_xy(x_vals, yt_meas)}"/>')

# Bottom plot: delta-theta vs t (centered around zero)
x0, y0 = plot2_origin
w, h = plot_w, plot_h
plot_svg += axes(x0, y0, w, h)
plot_svg.append(text(x0 - 18, y0 + 20, "Δθ", "label", "end"))
plot_svg.append(text(x0 + w + 18, y0 + h + 4, "t", "label"))
# zero line
plot_svg.append(
    f'<path class="zero" d="M {x0 + 10:.1f},{y0 + h / 2:.1f} H {x0 + w - 10:.1f}"/>'
)

# Map dtheta to y with symmetric range ~±max|dtheta|
rng = float(np.max(dtheta) - np.min(dtheta)) * 1.2
y_mid = y0 + h / 2
ys = y_mid - dtheta / rng * h
xs = x0 + w * t_norm
plot_svg.append(f'<path class="delta" d="{path_from_xy(xs, ys)}"/>')

# -------------------- LEGEND FOR RAYS --------------------
legend = []
legend.append(
    f'<line class="true" x1="{700:.1f}" y1="{70:.1f}" x2="{715:.1f}" y2="{55:.1f}"/>'
)
legend.append(
    f'<line class="measured" x1="{700:.1f}" y1="{100:.1f}" x2="{715:.1f}" y2="{85:.1f}"/>'
)

# -------------------- STYLE --------------------
style = """
<style>
    .ring { fill: none; stroke: #000; stroke-width: 3 }
    .ring.inner { stroke-width: 2 }
    .tick { stroke: #000; stroke-width: 3; stroke-linecap: round }
    .true { stroke: #ff33cc; stroke-width: 3; fill: none }
    .measured { stroke: #1e9d3a; stroke-width: 3; fill: none }
    .delta { stroke: #1e90ff; stroke-width: 4; fill: none }
    .zero { stroke: #888; stroke-width: 2; stroke-dasharray: 6 6; fill: none }
    line { stroke: #000; stroke-width: 2; fill: none }
    .axis { stroke: #000; stroke-width: 2; fill: none }
    .dotted { stroke: #000; stroke-width: 2; stroke-dasharray: 6 6; fill: none }
    .cross line { stroke: #000; stroke-width: 3; stroke-linecap: round }
    .cross.green line { stroke: #1e9d3a }
    .label { font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, "Noto Sans", "Apple Color Emoji", "Segoe UI Emoji"; font-size: 20px; fill: #000 }
    .label.green { fill: #1e9d3a }
    .label.label-true { fill: #ff33cc }
    .label.label-measured { fill: #1e9d3a }
</style>
"""

# -------------------- COMPOSE SVG --------------------
svg = []
svg.append(
    f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">'
)
svg.append(style)

# Left panel group
svg.append("<g>")
svg += ring_svg
svg += legend
svg.append("</g>")

# Right panel group
svg.append("<g>")
svg += plot_svg
svg.append("</g>")

svg.append("</svg>")
svg_str = "\n".join(svg)

# -------------------- WRITE FILE --------------------
out_path = Path("offset_encoder.svg")
out_path.write_text(svg_str, encoding="utf-8")
print(f"Wrote {out_path.resolve()}")
