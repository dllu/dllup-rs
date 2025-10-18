#!/usr/bin/env python3

import math
from pathlib import Path
import numpy as np

res_mult = 1.0
width = 600 * res_mult
height = 400 * res_mult
n_air = 1.000293
n_glass = 1.55

R = 90.0 * res_mult  # prism radius
thickness = 40.0 * res_mult  # center thickness
slope = 0.30  # face tilt factor (z = z0 + slope * (u·[xy]))
gap_z = 30.0 * res_mult
stroke = 2.2 * res_mult


def iso_project(p):
    return (
        np.array([width / 2, height / 2])
        + np.array([[np.sqrt(3) / 2, 0, -np.sqrt(3) / 2], [-0.5, 1, -0.5]]) @ p
        # + np.array([[0, 1, 0], [0, 0, 1]]) @ p
    )


def path_from_points(pts):
    d = [f"M{pts[-1][0]:.3f},{pts[-1][1]:.3f}"]
    for pt in pts:
        d.append(f"L{pt[0]:.3f},{pt[1]:.3f}")
    d.append("Z")
    return " ".join(d)


# ---------- Planes & ray tracing helpers ----------


def unit(v):
    v = np.asarray(v, float)
    n = np.linalg.norm(v)
    return v / n


def plane_from_tilt(z0, psi_deg, slope):
    """
    Elliptical face plane:
        z = z0 + slope * (u · [x,y])
    Put in n·p = d form with n = (-slope*u_x, -slope*u_y, 1), d = z0
    """
    u = np.array([math.cos(math.radians(psi_deg)), math.sin(math.radians(psi_deg))])
    n = np.array([-slope * u[0], -slope * u[1], 1.0])
    d = z0
    return unit(n), d


def plane_flat(z0, normal_sign=+1):
    # Flat face orthogonal to z (z = z0). normal_sign=+1 gives +z normal; -1 gives -z.
    n = np.array([0.0, 0.0, float(normal_sign)])
    d = z0
    return n, d


def intersect_ray_plane(p0, v, n, d):
    """
    Ray p(t) = p0 + t*v with plane n·p = d.
    Returns (t, p_hit). Assumes not parallel.
    """
    denom = float(np.dot(n, v))
    if abs(denom) < 1e-12:
        return None, None
    t = (d - float(np.dot(n, p0))) / denom
    return t, p0 + t * v


def refract_dir(v, n, n1, n2):
    """
    Vector Snell. 'n' must point INTO the incident medium.
    Handles both directions by flipping n and swapping indices as needed.
    Returns transmitted direction (unit) or None if TIR.
    """
    v = unit(v)
    n = unit(n)
    # Ensure n points into the incident medium
    if np.dot(v, n) > 0:
        n = -n
        n1, n2 = n2, n1
    eta = n1 / n2
    cosi = -float(np.dot(n, v))
    k = 1.0 - eta**2 * (1.0 - cosi**2)
    if k < 0.0:
        return None  # total internal reflection (won't happen here with gentle wedges)
    t = eta * v + (eta * cosi - math.sqrt(k)) * n
    return unit(t)


# ---------- Geometry for the two prisms (your original rendering) ----------


def prisms_paths(psi1_deg, psi2_deg):
    pts_prism_1_back = []
    pts_prism_1_front = []
    pts_prism_1_outline = []

    pts_prism_2_back = []
    pts_prism_2_front = []
    pts_prism_2_outline = []

    dir_prism_1 = np.array(
        [np.cos(math.radians(psi1_deg)), np.sin(math.radians(psi1_deg))]
    )
    dir_prism_2 = np.array(
        [np.cos(math.radians(psi2_deg)), np.sin(math.radians(psi2_deg))]
    )

    for i, angle in enumerate(np.linspace(0, np.pi * 2, 360)):
        x = np.cos(angle) * R
        y = np.sin(angle) * R

        # planes as z(x,y)
        front_z = np.dot(np.array([x, y]), dir_prism_1) * slope + thickness
        back_z = -np.dot(np.array([x, y]), dir_prism_2) * slope - thickness

        pts_prism_1_back.append(iso_project(np.array([x, y, front_z + gap_z / 2])))
        pts_prism_1_front.append(iso_project(np.array([x, y, gap_z / 2])))

        pts_prism_2_back.append(iso_project(np.array([x, y, -gap_z / 2])))
        pts_prism_2_front.append(iso_project(np.array([x, y, back_z - gap_z / 2])))

        # silhouette swap at ~±45° azimuth for isometric view
        if 180 - 45 < i <= 360 - 45:
            pts_prism_1_outline.append(pts_prism_1_back[-1])
            pts_prism_2_outline.append(pts_prism_2_back[-1])
        else:
            pts_prism_1_outline.append(pts_prism_1_front[-1])
            pts_prism_2_outline.append(pts_prism_2_front[-1])

    beam_path = trace_beam_polyline(psi1_deg, psi2_deg)
    out = [
        beam_path[0],
        f'<path class="face back" d="{path_from_points(pts_prism_1_back)}"/>',
        f'<path class="face outline" d="{path_from_points(pts_prism_1_outline)}"/>',
        f'<path class="face front" d="{path_from_points(pts_prism_1_front)}"/>',
        beam_path[1],
        f'<path class="face back" d="{path_from_points(pts_prism_2_back)}"/>',
        f'<path class="face outline magenta" d="{path_from_points(pts_prism_2_outline)}"/>',
        f'<path class="face front" d="{path_from_points(pts_prism_2_front)}"/>',
        beam_path[2],
    ]
    return out


# ---------- Red laser tracing through the prisms ----------


def trace_beam_polyline(psi1_deg, psi2_deg):
    # Ray starts well before the first prism on the optical axis, traveling +z.
    p = np.array([0.0, 0.0, 1000.0])
    v = np.array([0.0, 0.0, -1.0])

    beam01 = []
    beam12 = []
    beam23 = []

    beam01 = [p.copy()]  # collect 3D breakpoints

    # ----- Prism 1 -----
    # Back (tilted): z = gap/2 + thickness + slope*(u1·[x,y])
    n_b1, d_b1 = plane_from_tilt(+gap_z / 2.0 + thickness, psi1_deg, slope)
    _, p = intersect_ray_plane(beam01[-1], v, n_b1, d_b1)
    beam01.append(p)
    v = refract_dir(v, n_b1, n_air, n_glass)

    # Front (flat): z = +gap_z/2, normal pointing INTO air on the incident side is -z
    n_f1, d_f1 = plane_flat(+gap_z / 2.0, normal_sign=+1)
    _, p = intersect_ray_plane(beam01[-1], v, n_f1, d_f1)
    beam01.append(p)
    beam12.append(p.copy())
    v = refract_dir(v, n_f1, n_glass, n_air)

    # ----- Prism 2 -----
    # Back (flat): z = -gap/2, normal pointing into air on incident side is +z
    n_b2, d_b2 = plane_flat(-gap_z / 2.0, normal_sign=+1)
    _, p = intersect_ray_plane(beam12[-1], v, n_b2, d_b2)
    beam12.append(p)
    v = refract_dir(v, n_b2, n_air, n_glass)

    # Front (tilted): z = -gap/2 - thickness + slope*(u2·[x,y])
    n_f2, d_f2 = plane_from_tilt(-gap_z / 2.0 - thickness, psi2_deg, -slope)
    _, p = intersect_ray_plane(beam12[-1], v, n_f2, d_f2)
    beam12.append(p)
    beam23.append(p.copy())
    v = refract_dir(v, n_f2, n_glass, n_air)

    # ----- After prisms: extend beam forward so it exits the canvas
    p_end = beam23[-1] + 2000.0 * v
    beam23.append(p_end)

    print("final v:", v)

    # Project to 2D for SVG polyline

    paths = []
    for beam in (beam01, beam12, beam23):
        pts2d = [iso_project(q) for q in beam]
        d = f"M{pts2d[0][0]:.3f},{pts2d[0][1]:.3f} " + " ".join(
            f"L{q[0]:.3f},{q[1]:.3f}" for q in pts2d[1:]
        )
        paths.append(f'<path class="beam" d="{d}" />')
    return paths


# ---------- Main SVG ----------


def make_svg(
    psi1_deg=-90,
    psi2_deg=-90,
):
    shape_paths = prisms_paths(psi1_deg, psi2_deg)

    svg = (
        f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"
        viewBox="0 0 {width:.1f} {height:.1f}">
    <style>
    .face {{ fill: none; stroke: #111; stroke-width: {stroke}; vector-effect: non-scaling-stroke; }}
    .face.outline {{ fill: rgba(190, 200, 255, 0.5); stroke-width: {stroke};
                     vector-effect: non-scaling-stroke; }}
    .face.outline.magenta {{ fill: rgba(255, 160, 255, 0.5); stroke-width: {stroke};
                     vector-effect: non-scaling-stroke; }}
    .edge {{ stroke: #111; fill: none; stroke-width: {stroke}; vector-effect: non-scaling-stroke; }}
    .beam {{ stroke: #ff2a2a; stroke-width: 3.5; fill: none; stroke-linecap: round;
             vector-effect: non-scaling-stroke; }}
    </style>
    """
        + "\n".join(shape_paths)
        + "</svg>"
    )
    return svg


def main():
    psi_1 = -90
    psi_2 = 90
    mult = 2
    speed1 = 360 / 11 / mult  # deg / frame
    speed2 = -360 / 14 / mult  # deg / frame

    for frame in range(11 * 14 * mult):
        svg = make_svg(psi_1, psi_2)
        psi_1 += speed1
        psi_2 += speed2
        Path(f"risley_prisms_isometric_{frame:04d}.svg").write_text(
            svg, encoding="utf-8"
        )


if __name__ == "__main__":
    main()
