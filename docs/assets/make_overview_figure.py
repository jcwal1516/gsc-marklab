"""Render the README overview figure (light and dark) from a marklab run directory.

Usage:  python docs/assets/make_overview_figure.py <work_dir>
Expects <work_dir>/cells.csv, <work_dir>/mask.geojson and a completed run in
<work_dir>/run. Writes marklab-overview-{light,dark}.svg into <work_dir>.
All plotted values are read from the run's result.json and residual_territories.geojson.
"""

import csv
import json
import sys

THEMES = {
    "light": dict(
        bg="#ffffff",
        ink="#1f2328",
        muted="#59636e",
        grid="#d1d9e0",
        panel="#f6f8fa",
        un="#b8c2cc",
        mk="#cf3b52",
        terr="#8250df",
        curve="#0969da",
        env="#0969da",
        envop=".14",
    ),
    "dark": dict(
        bg="#0d1117",
        ink="#e6edf3",
        muted="#9198a1",
        grid="#30363d",
        panel="#151b23",
        un="#5a6672",
        mk="#ff6b81",
        terr="#bc8cff",
        curve="#58a6ff",
        env="#58a6ff",
        envop=".20",
    ),
}

W, H = 1180, 520
PAD = 28
AW = 560  # panel A width
AX, AY = PAD, 92
AH = H - AY - 46
BX, BY, BW, BH = AX + AW + 56, 92, 470, 170  # scale energy
CX, CY, CW, CH = BX, 320, 470, 150  # spectrum
TL = BX + 116  # panel B track left
TW = BW - 116 - 62  # panel B track width
FONT = "-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif"


def read_json(path):
    with open(path) as stream:
        return json.load(stream)


def read_cells(path):
    with open(path, newline="") as stream:
        return list(csv.DictReader(stream))


def esc(s):
    return str(s).replace("&", "&amp;").replace("<", "&lt;")


def track_x(value, lo, hi):
    return TL + (value - lo) / (hi - lo) * TW


def build(t, res, ring, cells, terr):
    xs = [p[0] for p in ring]
    ys = [p[1] for p in ring]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)

    def sx(v):
        return AX + (v - x0) / (x1 - x0) * AW

    def sy(v):
        return AY + AH - (v - y0) / (y1 - y0) * AH

    o = []
    a = o.append
    a(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" '
        f'height="{H}" font-family="{FONT}">'
    )
    a(f'<rect width="{W}" height="{H}" fill="{t["bg"]}"/>')

    # ---- header
    a(
        f'<text x="{PAD}" y="30" fill="{t["ink"]}" font-size="17" font-weight="600">'
        "marklab analyze</text>"
    )
    a(
        f'<text x="{PAD}" y="49" fill="{t["muted"]}" font-size="12.5">'
        f"{res['n_cells']} cells &#183; {res['n_marked']} marked "
        f"(p&#770; {res['p_hat']:.3f}) &#183; "
        "synthetic pattern, 999 permutations</text>"
    )

    # ---- Panel A: tissue
    a(
        f'<text x="{AX}" y="{AY - 10}" fill="{t["ink"]}" font-size="12.5" font-weight="600">'
        "Marked pattern and residual territories</text>"
    )
    ringpts = " ".join(f"{sx(p[0]):.1f},{sy(p[1]):.1f}" for p in ring)
    a(f'<polygon points="{ringpts}" fill="{t["panel"]}" stroke="{t["grid"]}" stroke-width="1.5"/>')
    for mark, radius, color in (("0", "1.5", t["un"]), ("1", "2.1", t["mk"])):
        for r in cells:
            if r["mark"] == mark:
                a(
                    f'<circle cx="{sx(float(r["x_um"])):.0f}" cy="{sy(float(r["y_um"])):.0f}" '
                    f'r="{radius}" fill="{color}"/>'
                )
    for f in terr["features"]:
        pts = " ".join(f"{sx(p[0]):.1f},{sy(p[1]):.1f}" for p in f["geometry"]["coordinates"][0])
        a(
            f'<polygon points="{pts}" fill="none" stroke="{t["terr"]}" stroke-width="1" '
            'opacity="0.55"/>'
        )
    ly = AY + AH + 20
    a(
        f'<circle cx="{AX + 5}" cy="{ly - 4}" r="3" fill="{t["mk"]}"/>'
        f'<text x="{AX + 15}" y="{ly}" fill="{t["muted"]}" font-size="11.5">marked</text>'
    )
    a(
        f'<circle cx="{AX + 88}" cy="{ly - 4}" r="3" fill="{t["un"]}"/>'
        f'<text x="{AX + 98}" y="{ly}" fill="{t["muted"]}" font-size="11.5">unmarked</text>'
    )
    a(
        f'<rect x="{AX + 186}" y="{ly - 8}" width="9" height="9" fill="none" '
        f'stroke="{t["terr"]}" stroke-width="1.4"/>'
        f'<text x="{AX + 201}" y="{ly}" fill="{t["muted"]}" font-size="11.5">'
        f"{len(terr['features'])} candidate territories</text>"
    )

    # ---- Panel B: scale energy vs global envelope (per-band bullet rows)
    se = res["scale_energy_curve"]
    p_se = res["scale_energy"]["value"]["p_global"]
    a(
        f'<text x="{BX}" y="{BY - 10}" fill="{t["ink"]}" font-size="12.5" font-weight="600">'
        "Scale energy vs permutation envelope</text>"
    )
    a(
        f'<text x="{BX + BW}" y="{BY - 10}" fill="{t["muted"]}" font-size="11.5" '
        f'text-anchor="end">p_global {p_se}</text>'
    )
    rh = BH / len(se)
    for i, b in enumerate(se):
        cy = BY + rh * (i + 0.5)
        lo_e = b["lower_global_envelope"]
        hi_e = b["upper_global_envelope"]
        obs = b["energy_fraction"]
        lo = min(lo_e, obs)
        hi = max(hi_e, obs)
        pad = (hi - lo) * 0.55 or 0.01
        lo -= pad
        hi += pad
        x_lo, x_hi, x_obs = (track_x(v, lo, hi) for v in (lo_e, hi_e, obs))
        a(
            f'<text x="{BX}" y="{cy + 1:.1f}" fill="{t["ink"]}" font-size="11.5">'
            f"{esc(b['band'])}</text>"
        )
        a(
            f'<text x="{BX}" y="{cy + 15:.1f}" fill="{t["muted"]}" font-size="10" '
            f'opacity=".85">{b["scale_um"]:.0f} &#181;m</text>'
        )
        a(
            f'<line x1="{TL}" y1="{cy:.1f}" x2="{TL + TW}" y2="{cy:.1f}" '
            f'stroke="{t["grid"]}" stroke-width="1"/>'
        )
        a(
            f'<rect x="{x_lo:.1f}" y="{cy - 11:.1f}" width="{max(x_hi - x_lo, 1.5):.1f}" '
            f'height="22" fill="{t["env"]}" opacity="{t["envop"]}"/>'
        )
        for x in (x_lo, x_hi):
            a(
                f'<line x1="{x:.1f}" y1="{cy - 11:.1f}" x2="{x:.1f}" y2="{cy + 11:.1f}" '
                f'stroke="{t["env"]}" stroke-width="1.2" opacity=".65"/>'
            )
        a(
            f'<line x1="{x_obs:.1f}" y1="{cy - 13:.1f}" x2="{x_obs:.1f}" y2="{cy + 13:.1f}" '
            f'stroke="{t["mk"]}" stroke-width="3"/>'
        )
        a(
            f'<text x="{TL + TW + 8}" y="{cy + 4:.1f}" fill="{t["ink"]}" font-size="11">'
            f"{obs:.3f}</text>"
        )
    ly2 = BY + BH + 16
    a(
        f'<rect x="{TL}" y="{ly2 - 8}" width="16" height="9" fill="{t["env"]}" '
        f'opacity="{t["envop"]}"/>'
    )
    a(f'<text x="{TL + 22}" y="{ly2}" fill="{t["muted"]}" font-size="10.5">global envelope</text>')
    a(
        f'<line x1="{TL + 128}" y1="{ly2 - 9}" x2="{TL + 128}" y2="{ly2 + 2}" '
        f'stroke="{t["mk"]}" stroke-width="3"/>'
    )
    a(
        f'<text x="{TL + 136}" y="{ly2}" fill="{t["muted"]}" font-size="10.5">'
        "observed (per-band scale)</text>"
    )

    # ---- Panel C: whitened spectrum
    sc = [p for p in res["spectrum_curve"] if p["inference_eligible"]]
    spectrum = res["spectrum"]["value"]
    a(
        f'<text x="{CX}" y="{CY - 10}" fill="{t["ink"]}" font-size="12.5" font-weight="600">'
        "Whitened spectrum</text>"
    )
    a(
        f'<text x="{CX + CW}" y="{CY - 10}" fill="{t["muted"]}" font-size="11.5" '
        f'text-anchor="end">low-k excess {spectrum["low_k_excess"]:.2f}</text>'
    )
    kmin, kmax = sc[0]["k"], sc[-1]["k"]
    wmax = max(max(p["whitened_power"] for p in sc), 1.0)

    def cxp(k):
        return CX + (k - kmin) / (kmax - kmin) * CW

    def cyp(v):
        return CY + CH - (v / wmax) * CH

    a(
        f'<line x1="{CX}" y1="{cyp(1.0):.1f}" x2="{CX + CW}" y2="{cyp(1.0):.1f}" '
        f'stroke="{t["grid"]}" stroke-dasharray="3 3"/>'
    )
    a(
        f'<text x="{CX + CW - 2}" y="{cyp(1.0) - 5:.1f}" fill="{t["muted"]}" font-size="10" '
        'text-anchor="end">null median</text>'
    )
    pl = " ".join(f"{cxp(p['k']):.1f},{cyp(p['whitened_power']):.1f}" for p in sc)
    a(
        f'<polyline points="{pl}" fill="none" stroke="{t["curve"]}" stroke-width="2" '
        'stroke-linejoin="round"/>'
    )
    a(
        f'<circle cx="{cxp(sc[0]["k"]):.1f}" cy="{cyp(sc[0]["whitened_power"]):.1f}" r="3.4" '
        f'fill="{t["mk"]}"/>'
    )
    a(f'<line x1="{CX}" y1="{CY + CH}" x2="{CX + CW}" y2="{CY + CH}" stroke="{t["grid"]}"/>')
    a(f'<text x="{CX}" y="{CY + CH + 16}" fill="{t["muted"]}" font-size="10.5">low k</text>')
    a(
        f'<text x="{CX + CW}" y="{CY + CH + 16}" fill="{t["muted"]}" font-size="10.5" '
        'text-anchor="end">high k</text>'
    )
    a(
        f'<text x="{CX}" y="{CY + CH + 33}" fill="{t["muted"]}" font-size="10.5">'
        f"max interpretable scale {spectrum['max_interpretable_scale_um']:.0f} &#181;m</text>"
    )

    a("</svg>")
    return "".join(o)


def main(work_dir):
    run = f"{work_dir}/run"
    res = read_json(f"{run}/result.json")["analysis"]["result"]
    ring = read_json(f"{work_dir}/mask.geojson")["coordinates"][0][0]
    terr = read_json(f"{run}/residual_territories.geojson")
    cells = read_cells(f"{work_dir}/cells.csv")
    for name, theme in THEMES.items():
        svg = build(theme, res, ring, cells, terr)
        with open(f"{work_dir}/marklab-overview-{name}.svg", "w") as stream:
            stream.write(svg)
        print("wrote", f"marklab-overview-{name}.svg")


if __name__ == "__main__":
    main(sys.argv[1])
