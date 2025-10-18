#!/usr/bin/env python3
"""
Generate an SVG diagram of two Risley-prism pairs.

Requirements from user:
- Prisms are right triangles with the purely vertical edges facing each other.
- Beam kinks *exactly* on the glass faces via Snell's law (air n=1.0, glass n=1.5).
- No labels or arrowheads.
- Incident laser beam centered in each pair.
- Style:
    .prism { fill:#ddd; stroke:#8a8f98; stroke-width:2; vector-effect:non-scaling-stroke }
    .beam  { stroke:#ff2a2a; stroke-width:4; stroke-linecap:round; vector-effect:non-scaling-stroke }
"""

import math
from typing import List, Tuple

Pt = Tuple[float, float]


# ---------- math helpers ----------
def normalize(v: Pt) -> Pt:
    x, y = v
    n = math.hypot(x, y)
    return (x / n, y / n)


def cross(a: Pt, b: Pt) -> float:
    return a[0] * b[1] - a[1] * b[0]


def face_normal(a: Pt, b: Pt) -> Pt:
    # +90° rotation of edge vector gives one of the two valid normals.
    t = (b[0] - a[0], b[1] - a[1])
    n = (-t[1], t[0])
    nlen = math.hypot(*n)
    return (n[0] / nlen, n[1] / nlen)


def intersect_ray_segment(p: Pt, d: Pt, a: Pt, b: Pt, eps: float = 1e-9) -> Pt:
    """Intersection point of ray p + t d (t>=0) with segment a->b (0<=u<=1)."""
    s = (b[0] - a[0], b[1] - a[1])
    denom = cross(d, s)
    if abs(denom) < eps:
        raise RuntimeError("Ray is parallel to the segment.")
    ap = (a[0] - p[0], a[1] - p[1])
    t = cross(ap, s) / denom
    u = cross(ap, d) / denom
    if t < -eps or u < -eps or u > 1 + eps:
        raise RuntimeError("Ray does not hit the segment in front of the origin.")
    return (p[0] + t * d[0], p[1] + t * d[1])


def refract(incident: Pt, N: Pt, n1: float, n2: float, eps: float = 1e-9) -> Pt:
    """
    Snell's law in vector form.
    I = incident direction (unit), pointing *toward* the surface.
    N = surface normal (unit). If not oriented into the incident medium, it’s flipped.
    Returns transmitted (refracted) unit direction. Raises on total internal reflection.
    """
    Ix, Iy = normalize(incident)
    Nx, Ny = normalize(N)

    cosi = -(Ix * Nx + Iy * Ny)
    if cosi < 0:  # ensure N points into the incident side
        Nx, Ny = -Nx, -Ny
        cosi = -(Ix * Nx + Iy * Ny)

    eta = n1 / n2
    k = 1 - eta * eta * (1 - cosi * cosi)
    if k < -eps:
        raise RuntimeError("Total internal reflection in this configuration.")
    cost = math.sqrt(max(0.0, k))

    tx = eta * Ix + (eta * cosi - cost) * Nx
    ty = eta * Iy + (eta * cosi - cost) * Ny
    return normalize((tx, ty))


# ---------- geometry builders ----------
def make_right_wedge(
    x_inner: float,
    y_top: float,
    y_bottom: float,
    width: float,
    *,
    side: str,
    slope: str,
) -> Tuple[List[Pt], Tuple[Pt, Pt], Tuple[Pt, Pt]]:
    """
    Build a right triangular prism (2D cross-section).
    - The purely vertical edge is at x_inner from y_top to y_bottom (faces the gap).
    - 'side'  : 'left' or 'right' prism of the pair.
    - 'slope' : 'down' -> slanted face goes from inner top to outer bottom
                'up'   -> slanted face goes from inner bottom to outer top
    Returns: (triangle_points, slanted_face_segment, inner_vertical_face_segment)
    """
    if side == "left":
        if slope == "down":
            pts = [(x_inner, y_top), (x_inner, y_bottom), (x_inner - width, y_bottom)]
            slanted = (pts[0], pts[2])
        else:  # slope 'up'
            pts = [(x_inner, y_top), (x_inner, y_bottom), (x_inner - width, y_top)]
            slanted = (pts[1], pts[2])
        vertical = (pts[0], pts[1])
    else:  # right prism
        if slope == "down":
            pts = [(x_inner, y_top), (x_inner, y_bottom), (x_inner + width, y_bottom)]
            slanted = (pts[0], pts[2])
        else:  # slope 'up'
            pts = [(x_inner, y_top), (x_inner, y_bottom), (x_inner + width, y_top)]
            slanted = (pts[1], pts[2])
        vertical = (pts[0], pts[1])
    return pts, slanted, vertical


def trace_pair(
    y_center: float,
    y_top: float,
    y_bottom: float,
    x_left_inner: float,
    gap: float,
    width: float,
    *,
    co_rotate: bool,
    n_air: float = 1.0,
    n_glass: float = 1.5,
    endx: float = 870.0,
) -> Tuple[List[Pt], List[Pt], List[Pt]]:
    """
    Trace beam through a pair of wedges.
    - If co_rotate=True, wedges add deviation; else they counter-rotate to (nearly) cancel.
    Returns (beam_points, left_triangle_pts, right_triangle_pts)
    """
    x_right_inner = x_left_inner + gap

    L_pts, L_slanted, L_vertical = make_right_wedge(
        x_left_inner, y_top, y_bottom, width, side="left", slope="down"
    )
    R_pts, R_slanted, R_vertical = make_right_wedge(
        x_right_inner,
        y_top,
        y_bottom,
        width,
        side="right",
        slope=("down" if co_rotate else "up"),
    )

    beam: List[Pt] = [(50.0, y_center)]  # centered incoming ray
    d = (1.0, 0.0)

    # Enter left prism through slanted face
    p = intersect_ray_segment(beam[-1], d, *L_slanted)
    beam.append(p)
    d = refract(d, face_normal(*L_slanted), n_air, n_glass)

    # Exit left prism at inner vertical face (into the gap)
    p = intersect_ray_segment(beam[-1], d, *L_vertical)
    beam.append(p)
    d = refract(d, face_normal(*L_vertical), n_glass, n_air)

    # Enter right prism at its inner vertical face
    p = intersect_ray_segment(beam[-1], d, *R_vertical)
    beam.append(p)
    d = refract(d, face_normal(*R_vertical), n_air, n_glass)

    # Exit right prism at its slanted face
    p = intersect_ray_segment(beam[-1], d, *R_slanted)
    beam.append(p)
    d = refract(d, face_normal(*R_slanted), n_glass, n_air)

    # Continue to the right edge of the canvas
    t_end = (endx - p[0]) / d[0]
    beam.append((endx, p[1] + t_end * d[1]))

    return beam, L_pts, R_pts


# ---------- build the scene ----------
def fmt_points(pts: List[Pt]) -> str:
    return " ".join(f"{x:.2f},{y:.2f}" for x, y in pts)


# Layout parameters
VIEW_W, VIEW_H = 920, 640
top_top, top_bottom = 100.0, 220.0
bottom_top, bottom_bot = 380.0, 500.0
center_top = (top_top + top_bottom) / 2
center_bottom = (bottom_top + bottom_bot) / 2
x_left_inner, gap, width = 420.0, 60.0, 60.0  # feel free to tweak

# Trace both rows
beam_top, L1, R1 = trace_pair(
    center_top, top_top, top_bottom, x_left_inner, gap, width, co_rotate=True
)
beam_bot, L2, R2 = trace_pair(
    center_bottom, bottom_top, bottom_bot, x_left_inner, gap, width, co_rotate=False
)

# ---------- output SVG ----------
svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEW_W} {VIEW_H}" width="{VIEW_W}" height="{VIEW_H}">
  <style>
    .prism {{ fill:#ddd; stroke:#8a8f98; stroke-width:2; vector-effect:non-scaling-stroke }}
    .beam  {{ fill:none; stroke:#ff2a2a; stroke-width:4; stroke-linecap:round; vector-effect:non-scaling-stroke }}
  </style>

  <!-- Top pair -->
  <polygon class="prism" points="{fmt_points(L1)}"/>
  <polygon class="prism" points="{fmt_points(R1)}"/>
  <polyline class="beam" points="{fmt_points(beam_top)}"/>

  <!-- Bottom pair -->
  <polygon class="prism" points="{fmt_points(L2)}"/>
  <polygon class="prism" points="{fmt_points(R2)}"/>
  <polyline class="beam" points="{fmt_points(beam_bot)}"/>
</svg>
"""

print(svg)
