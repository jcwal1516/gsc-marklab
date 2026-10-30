"""Validate `marklab spectrum preprocess` on the RKI MALDI-TOF database v4.2 (Zenodo 14562231).

Usage, from the repository root:

    python3 -I tests/fixtures/maldi/rki_checks.py target/debug/marklab DB_ROOT \
        tests/fixtures/maldi/ecoli_k12_ribosomal_masses.tsv OUT_DIR

DB_ROOT is the extracted database; OUT_DIR must not exist.

Declared before running (2026-10-28):

* Coverage (check 5): read every acquisition (fid with acqu) with no steps. Report accepted
  acquisitions and every rejection reason with its count.
* Vendor calibration (check 3): for each acquisition with pdata/1/peaklist.xml written by
  flexAnalysis, linearly interpolate marklab's m/z at each peak's fractional data-point index
  `ind` and compare with flexAnalysis's `mass`. Both 0-based and 1-based index conventions are
  reported; the convention is identified, not tuned. Pass if the median absolute difference
  over all peaks is at most 1 ppm for one convention.
* Ribosomal masses (check 4): for every accepted Escherichia coli acquisition, run sqrt,
  Savitzky-Golay (10, 3), SNIP (20, decreasing), total ion current and trim 3,000-12,000 Da.
  For each E. coli K-12 ribosomal protein [M+H]+ average mass computed from UniProt sequences,
  take the intensity-weighted centroid of the contiguous points above half the apex around the
  highest point within +-1,000 ppm, counted only when the apex exceeds 5 times the median
  intensity of the window +-2% around the expected mass. Pass if, for each protein detected in at
  least half of the spectra, the median error is within +-500 ppm (linear-mode external
  calibration).
"""

import collections
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile

MARKLAB, ROOT, EXPECTED, OUT = sys.argv[1:5]
os.makedirs(OUT, exist_ok=False)
CONFIG_EMPTY = os.path.join(OUT, "empty.json")
CONFIG_PIPELINE = os.path.join(OUT, "pipeline.json")
with open(CONFIG_EMPTY, "w") as handle:
    json.dump({"steps": []}, handle)
with open(CONFIG_PIPELINE, "w") as handle:
    json.dump(
        {
            "steps": [
                {"step": "sqrt"},
                {"step": "savitzky_golay", "half_window": 10, "polynomial_order": 3},
                {"step": "snip_baseline", "iterations": 20, "decreasing": True},
                {"step": "total_ion_current"},
                {"step": "trim", "min_mz": 3000, "max_mz": 12000},
            ]
        },
        handle,
    )


def run(acquisition, config):
    with tempfile.TemporaryDirectory(dir=OUT) as scratch:
        out = os.path.join(scratch, "result.json")
        done = subprocess.run(
            [MARKLAB, "spectrum", "preprocess", "--bruker", acquisition, "--config", config,
             "--out", out],
            capture_output=True, text=True,
        )
        if done.returncode != 0:
            return None, done.stderr.strip().splitlines()[-1] if done.stderr.strip() else "no message"
        with open(out) as handle:
            return json.load(handle), None


def reason(message):
    message = re.sub(r"\d+(\.\d+)?", "N", message)
    return message.split("Bruker flex acquisition: ")[-1][:120]


def interpolate(xs, index):
    lower = int(index)
    if lower < 0 or lower + 1 >= len(xs):
        return None
    fraction = index - lower
    return xs[lower] + fraction * (xs[lower + 1] - xs[lower])


acquisitions = sorted(
    dirpath for dirpath, _, files in os.walk(ROOT) if "fid" in files and "acqu" in files
)
expected = [
    (float(row.split("\t")[0]), row.split("\t")[1])
    for row in open(EXPECTED).read().splitlines()[1:]
]

coverage = collections.Counter()
vendor = {"0-based": [], "1-based": []}
vendor_spectra = 0
ribosomal = collections.defaultdict(list)
ecoli_spectra = 0
for number, acquisition in enumerate(acquisitions, 1):
    result, error = run(acquisition, CONFIG_EMPTY)
    if error:
        coverage["rejected: " + reason(error)] += 1
        continue
    coverage["accepted"] += 1
    mz = result["spectrum"]["mz"]

    peaklist = os.path.join(acquisition, "pdata", "1", "peaklist.xml")
    if os.path.isfile(peaklist):
        text = open(peaklist, "rb").read().decode("utf-8", "replace")
        peaks = re.findall(r"<pk>.*?<ind>([^<]+)</ind>.*?<mass>([^<]+)</mass>.*?</pk>", text, re.S)
        if peaks:
            vendor_spectra += 1
        for ind, mass in peaks:
            ind, mass = float(ind), float(mass)
            for label, offset in (("0-based", 0.0), ("1-based", -1.0)):
                ours = interpolate(mz, ind + offset)
                if ours is not None:
                    vendor[label].append((ours - mass) / mass * 1e6)

    if "Escherichia coli" in acquisition:
        processed, error = run(acquisition, CONFIG_PIPELINE)
        if error:
            coverage["E. coli pipeline failed: " + reason(error)] += 1
            continue
        ecoli_spectra += 1
        xs, ys = processed["spectrum"]["mz"], processed["spectrum"]["intensity"]
        for mass, gene in expected:
            window = [i for i, x in enumerate(xs) if abs(x - mass) <= 1e-3 * mass]
            context = [ys[i] for i, x in enumerate(xs) if abs(x - mass) <= 0.02 * mass]
            if not window or not context:
                continue
            apex = max(window, key=lambda i: ys[i])
            if ys[apex] <= 5 * statistics.median(context):
                continue
            half = ys[apex] / 2
            left, right = apex, apex
            while left > 0 and ys[left - 1] > half:
                left -= 1
            while right + 1 < len(ys) and ys[right + 1] > half:
                right += 1
            weight = sum(ys[left:right + 1])
            centroid = sum(xs[i] * ys[i] for i in range(left, right + 1)) / weight
            ribosomal[(mass, gene)].append((centroid - mass) / mass * 1e6)
    if number % 1000 == 0:
        print(f"{number}/{len(acquisitions)}", flush=True)

report = {"acquisitions": len(acquisitions), "coverage": dict(coverage)}
report["vendor"] = {"spectra": vendor_spectra}
for label, values in vendor.items():
    if values:
        absolute = sorted(abs(v) for v in values)
        report["vendor"][label] = {
            "peaks": len(values),
            "median_abs_ppm": statistics.median(absolute),
            "p99_abs_ppm": absolute[int(0.99 * (len(absolute) - 1))],
            "max_abs_ppm": absolute[-1],
            "median_signed_ppm": statistics.median(values),
        }
report["vendor"]["pass"] = any(
    report["vendor"].get(label, {}).get("median_abs_ppm", 1e9) <= 1.0 for label in vendor
)
report["ribosomal"] = {"ecoli_spectra": ecoli_spectra, "proteins": []}
passes = []
for (mass, gene), errors in sorted(ribosomal.items()):
    detected = len(errors) / ecoli_spectra if ecoli_spectra else 0.0
    quartiles = statistics.quantiles(errors, n=4) if len(errors) >= 2 else [errors[0]] * 3
    entry = {
        "gene": gene, "expected_mh": mass, "detected_fraction": round(detected, 3),
        "median_ppm": round(statistics.median(errors), 1),
        "iqr_ppm": [round(quartiles[0], 1), round(quartiles[2], 1)],
    }
    report["ribosomal"]["proteins"].append(entry)
    if detected >= 0.5:
        passes.append(abs(entry["median_ppm"]) <= 500)
report["ribosomal"]["pass"] = bool(passes) and all(passes)
with open(os.path.join(OUT, "report.json"), "w") as handle:
    json.dump(report, handle, indent=2)
print(json.dumps(report, indent=2))
