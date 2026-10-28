# MALDI-TOF spectrum preprocessing

`marklab microbio preprocess` reads one MALDI-TOF spectrum of a bacterial isolate, either a Bruker
flex-series acquisition (converting time of flight to m/z) or a two-column text spectrum, applies
declared preprocessing steps in order, and can sum the result into fixed m/z bins. It is the first
stage of the microbiology workflow. It does not identify species, detect peaks or predict
antimicrobial susceptibility, and none of Marklab's spatial nulls apply to spectra.

```sh
marklab microbio preprocess \
  --bruker run/0_A1/1/1SLin \
  --config examples/maldi-preprocess.json \
  --out out/0_A1.json

marklab microbio preprocess \
  --text raw/<code>.txt \
  --config examples/maldi-preprocess.json \
  --out out/<code>.json
```

Exactly one of `--bruker` and `--text` is required.

## Bruker flex input

`--bruker` names the spectrum directory holding `fid` and `acqu`. As in readBrukerFlexData,
`acqus` is used when `acqu` is absent.

| `acqu` parameter | Requirement |
|---|---|
| `BYTORDA` | `0` (little-endian `fid`) or `1` (big-endian) |
| `TD` | Number of time points, 1 to 4,194,304; `fid` must hold exactly `4 TD` bytes of signed 32-bit counts |
| `DELAY`, `DW` | Finite; `DW > 0` |
| `ML1`, `ML2`, `ML3` | Finite calibration constants; `ML1 > 0` |
| `NoSHOTS` | At least 1 |
| `AQOP_m` | Optional: `0` linear, `1` reflector |

Times of flight are `tof_i = DELAY + i DW`. Each m/z value `m` solves the flex-series calibration
`tof = ML2 + sqrt(1e12 / ML1) sqrt(m) + ML3 m` (Titulaer et al., *BMC Bioinformatics* 2006), the
same conversion readBrukerFlexData uses, evaluated in a form that avoids cancellation when `ML3` is
small. Negative raw counts are set to zero, as readBrukerFlexData
does by default, and their number is reported. The resulting m/z values must be positive and
strictly increasing.

The reader rejects, rather than approximates:

- Bruker high-precision calibration (`HPClUse = yes` with positive `HPClBLo`, `HPClBHi`, `HPClOrd`);
- LIFT acquisitions (non-zero `Lift1` and `Lift2`, or non-zero `TLift`) and spectrum types other
  than TOF (`SPType` other than 0);
- `V1.0CTOF2CalibrationConstants` in `NTBCal`;
- a missing, repeated or non-finite parameter among those read, and a `fid` whose length differs
  from `4 TD`.

readBrukerFlexData only warns about HPC and LIFT; for LIFT it returns times of flight in place of
masses. `acqu` lines may end in LF, CRLF or CR, and a decimal comma is read as a point. `acqu` may
be at most 1 MiB.

## Text input

`--text` names a spectrum already in m/z, such as a DRIAMS `raw` or `preprocessed` file. Lines
starting with `#` and blank lines are ignored, and the first remaining line may be a header of two
non-numeric fields. Every other line holds m/z and a non-negative intensity separated by
whitespace, with m/z strictly increasing; any other line is rejected with its line number. The file
may be at most 128 MiB and hold at most 4,194,304 points.

## Configuration

The JSON configuration lists `steps` in the order they run and may declare `binning`. Unknown
steps and fields are rejected.

```json
{
  "steps": [
    {"step": "sqrt"},
    {"step": "savitzky_golay", "half_window": 10, "polynomial_order": 3},
    {"step": "snip_baseline", "iterations": 20, "decreasing": true},
    {"step": "total_ion_current"},
    {"step": "trim", "min_mz": 2000, "max_mz": 20000}
  ],
  "binning": {"min_mz": 2000, "max_mz": 20000, "bins": 6000}
}
```

| Step | Definition | MALDIquant equivalent |
|---|---|---|
| `sqrt` | `I ← √I` | `transformIntensity(method = "sqrt")` |
| `savitzky_golay` | Least-squares polynomial of degree `polynomial_order` over `2 half_window + 1` points. Interior points use the centred window; the first and last `half_window` points use the first or last full window evaluated at their own position. Negative results are set to zero. Requires `1 ≤ half_window ≤ 500`, `polynomial_order ≤ 10` and `polynomial_order < 2 half_window + 1 ≤ n`. | `smoothIntensity(method = "SavitzkyGolay", halfWindowSize = h)`; MALDIquant's order defaults to 3 |
| `snip_baseline` | For each clipping half-window `w`, from `iterations` down to 1 when `decreasing`, otherwise from 1 up: `b_j ← min(b_j, (b_{j−w} + b_{j+w}) / 2)` for `w ≤ j < n − w`, every point computed from the previous pass. Then `I ← I − b`. Requires `1 ≤ iterations ≤ 1000`. | `removeBaseline(method = "SNIP", iterations, decreasing)` |
| `total_ion_current` | `I ← I / A` with `A = Σ (I_j + I_{j+1}) / 2 · (m_{j+1} − m_j)`. Fails unless `A` is finite and positive. | `calibrateIntensity(method = "TIC")`, which instead warns and leaves intensities unchanged when `A = 0` |
| `trim` | Keep points with `min_mz ≤ m ≤ max_mz`. Fails when none remain. | `trim(range = c(min_mz, max_mz))` |

Order matters. Total ion current before `trim` divides by the area of the whole spectrum; after
`trim`, by the area inside the range.

Bins have edges `e_i = min_mz + i (max_mz − min_mz) / bins`, with the last edge exactly
`max_mz`. Bin `i` covers `[e_i, e_{i+1})` and the last bin also includes `max_mz`. These are the
boundaries of `numpy.histogram` over `numpy.linspace` edges, which maldi-learn's
`BinningVectorizer` used to produce the DRIAMS `binned_6000` files (2,000–20,000 Da, 6,000 bins of
3 Da). Each bin sums its own points in m/z order, and points outside `[min_mz, max_mz]` are counted
in `points_outside`. Given explicit edges, NumPy instead differences cumulative sums, so a bin
beside much larger intensities can differ from Marklab's by rounding relative to the running total.
Unlike maldi-learn, Marklab keeps a point exactly at `min_mz` and never moves the edges;
maldi-learn recomputes edges for a spectrum that extends beyond 2,000–20,000 Da. At most 1,000,000
bins are allowed, and a non-finite bin width or bin sum is rejected.

The example configuration uses MALDIquant's smoothing defaults, 20 SNIP iterations, total ion
current before trimming, and the DRIAMS bin layout.

## Output

The result JSON contains:

- `source`: for Bruker input, `format` (`bruker_flex`), `time_points`, `byte_order`,
  `laser_shots`, `tof_mode` and `negative_intensities_clipped`; for text input, `format` (`text`)
  and `points`;
- `steps`: the steps as run;
- `spectrum`: the preprocessed `mz` and `intensity` arrays;
- `binned`: `min_mz`, `max_mz`, `bins`, `points_outside` and `intensity`, or `null` without
  `binning`.

The output file must not already exist.

## Evidence

Unit tests cover known calibrations (the linear case exactly and the quadratic case by inverting
the calibration equation), the classic five-point quadratic Savitzky–Golay filter, exact
preservation of cubics, SNIP's previous-pass update, the trapezoidal total ion current, the
inclusive trim, bin edges, and each rejection above. A CLI test runs a synthetic acquisition end to
end. The fuzz target `bruker_flex` exercises the parser.

[`tests/fixtures/maldi/compare_maldiquant.R`](../tests/fixtures/maldi/compare_maldiquant.R)
compares the command with readBrukerFlexData 1.9.3 and MALDIquant 1.22.3 (R 4.5.2) on the two
linear example acquisitions shipped with readBrukerFlexData (22,431 points each), after every step
and with both SNIP directions. The tolerances were fixed before the first run.

| Comparison | Largest difference |
|---|---|
| m/z | 2.0e-12 relative (tolerance 1e-9) |
| Raw intensities | identical |
| Intensities after each step | 2.8e-12 of the spectrum maximum (tolerance 1e-9) |
| 6,000-bin vectors against a per-bin R sum | 2.8e-12 of the largest bin (tolerance 1e-9) |
| High-precision-calibration example | rejected as required |

The m/z differences come from readBrukerFlexData's form of the root, which loses digits to
cancellation for these calibrations. The intensity differences enter through the total ion
current, which integrates over m/z; when both used the same root form, intensities agreed within
3.4e-14.

These spectra test numerical agreement with MALDIquant. They are not a cohort of isolates.

## Not yet supported

- More than one spectrum per run, replicate averaging, peak detection and alignment.
- mzML and mzXML.
- Bruker HPC, LIFT and PSD acquisitions.
