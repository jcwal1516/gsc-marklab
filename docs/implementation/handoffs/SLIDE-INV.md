# Task Handoff — SLIDE-INV

Status: complete
Base SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24`
Final SHA: `55fce12f10684a9081ca1f744f87d6f5feedcb24` (read-only task)
Branch/worktree: `branch/frontier-transformation` / `/Users/user/Bench/gsc-marklab`
Owner: `remote_slide_inventory`

## Scope delivered

Parent requirement IDs: FND-01, FND-02, FND-05, EMB-CORE, WS-24, C-04

Scientific behavior: read-only, privacy-safe inventory of authorized remote WSI, sidecars, CellViT outputs, embedding matrices, patch assets, and provenance. The task established asset availability and gaps; it did not ingest data or validate a Marklab scientific result.

Explicit non-goals preserved: no remote mutation; no patient data copied into the repository; no `.pt`/pickle deserialization; no claim that prepared derivatives are unique biological slides; no patch-embedding claim.

## Changed files

- Repository: none.
- Remote: none.
- Local scratch `/tmp/marklab-slide-inventory.FohXVF` contained only initial public-research PDF candidates from before the WSI scope correction and was moved to Trash; it is recoverable until Trash is emptied. No patient data was present.

## Changed canonical symbols

- None.

Registry updated: not applicable; the lead recorded asset contracts and blockers in the implementation ledgers.

## Commands actually run

| Command group | Exit/result | Notes |
|---|---|---|
| Authenticated SSH/host identity | success | Existing BatchMode-authenticated connection; no password used or exposed |
| WSI/TIFF inventory | success | Aggregate paths/counts/bytes only; identifier-valued output omitted |
| Sidecar/modality inventory | success | Aggregate extension counts/bytes |
| Manifest/checkpoint hashes | success | Selected full SHA-256 values recorded |
| NPY/snappy/HDF5 inspection | success | Safe headers/schema/metadata only; no pickle |
| Repository scope | success | `git status --short`; `git diff --check` |
| Scratch cleanup | success | `trash /tmp/marklab-slide-inventory.FohXVF` |

Exact connection and host commands:

```bash
ssh -o BatchMode=yes -o ConnectTimeout=8 -o ConnectionAttempts=1 100.82.140.72 'printf "%s\n" "CONNECTED"; uname -s; pwd'
ssh 100.82.140.72 'scutil --get ComputerName 2>/dev/null; hostname; df -h | sed -n "1,20p"'
```

Exact aggregate WSI command:

```bash
ssh 100.82.140.72 'find /Users/user/Bench /Volumes/1TB /Volumes/500GB \
  \( -path "*/.git" -o -path "*/target" -o -path "*/node_modules" \
     -o -path "*/env" -o -path "*/.venv" -o -path "*/venv" \
     -o -path "*/site-packages" -o -path "*/.Trashes" -o -path "*/.Trash" \) \
  -prune -o -type f \
  \( -iname "*.svs" -o -iname "*.ndpi" -o -iname "*.mrxs" \
     -o -iname "*.scn" -o -iname "*.vms" -o -iname "*.vmu" \
     -o -iname "*.bif" -o -iname "*.czi" -o -iname "*.ome.tif" \
     -o -iname "*.ome.tiff" -o -iname "*.tif" -o -iname "*.tiff" \
     -o -iname "*.dcm" -o -iname "*.dicom" -o -iname "*.jp2" \
     -o -iname "*.j2k" \) \
  -exec stat -f "%z|%m|%N" {} + 2>/dev/null' |
awk -F'|' '
function ext(path, low) {
  low=tolower(path)
  if (low ~ /\.ome\.tiff$/) return "ome.tiff"
  if (low ~ /\.ome\.tif$/) return "ome.tif"
  sub(/^.*\./,"",low)
  return low
}
function root(path) {
  if (index(path,"/Users/user/Bench/")==1) return "home/Bench"
  if (index(path,"/Volumes/1TB/")==1) return "volume/1TB"
  if (index(path,"/Volumes/500GB/")==1) return "volume/500GB"
  return "other"
}
{
  e=ext($3); r=root($3); k=r "|" e
  n[k]++; bytes[k]+=$1
  if (!(k in oldest) || $2<oldest[k]) oldest[k]=$2
  if (!(k in newest) || $2>newest[k]) newest[k]=$2
}
END {
  for (k in n)
    printf "%s|%d|%.0f|%d|%d\n",k,n[k],bytes[k],oldest[k],newest[k]
}' | sort
```

Exact large-TIFF command:

```bash
ssh 100.82.140.72 'find /Users/user/Bench /Volumes/1TB /Volumes/500GB \
  \( -path "*/.git" -o -path "*/target" -o -path "*/node_modules" \
     -o -path "*/env" -o -path "*/.venv" -o -path "*/venv" \
     -o -path "*/site-packages" -o -path "*/.Trashes" -o -path "*/.Trash" \) \
  -prune -o -type f \( -iname "*.tif" -o -iname "*.tiff" \) -size +100M \
  -exec stat -f "%z|%m|%N" {} + 2>/dev/null' |
awk -F'|' '
function root(path) {
  if (index(path,"/Users/user/Bench/")==1) return "home/Bench"
  if (index(path,"/Volumes/1TB/")==1) return "volume/1TB"
  if (index(path,"/Volumes/500GB/")==1) return "volume/500GB"
  return "other"
}
{
  r=root($3); n[r]++; bytes[r]+=$1
  if (!(r in oldest) || $2<oldest[r]) oldest[r]=$2
  if (!(r in newest) || $2>newest[r]) newest[r]=$2
}
END {
  for (r in n)
    printf "%s|%d|%.0f|%d|%d\n",r,n[r],bytes[r],oldest[r],newest[r]
}' | sort
```

Exact MRXS companion command:

```bash
ssh 100.82.140.72 '
find /Users/user/Bench/CellViT-plus-plus/results/public_validation/stage3_crc_imc_external_v1/pilot_stage/wsis \
  -maxdepth 1 -type f -iname "*.mrxs" -print0 2>/dev/null |
while IFS= read -r -d "" f; do
  stem=${f%.mrxs}
  if [ -d "$stem" ]; then
    du -sk "$stem"
  else
    printf "MISSING\t%s\n" "$f"
  fi
done' |
awk '
BEGIN { n=0; kb=0; missing=0 }
$1=="MISSING" { missing++; next }
{ n++; kb+=$1 }
END { printf "companions=%d bytes=%.0f missing=%d\n",n,kb*1024,missing }'
```

Exact sidecar command:

```bash
ssh 100.82.140.72 '
find /Users/user/Bench/CellViT-plus-plus/results/public_validation \
     /Volumes/1TB/marklab /Volumes/500GB/marklab \
  \( -path "*/.git" -o -path "*/target" -o -path "*/node_modules" \
     -o -path "*/env" -o -path "*/.venv" -o -path "*/venv" \
     -o -path "*/site-packages" -o -path "*/.Trashes" \) \
  -prune -o -type f \
  \( -iname "*.geojson" -o -iname "*.csv" -o -iname "*.tsv" \
     -o -iname "*.parquet" -o -iname "*.feather" -o -iname "*.arrow" \
     -o -iname "*.h5" -o -iname "*.hdf5" -o -iname "*.h5ad" \
     -o -iname "*.npy" -o -iname "*.npz" -o -iname "*.pth" \
     -o -iname "*.pt" -o -iname "*.ckpt" -o -iname "*.safetensors" \
     -o -iname "*.onnx" \) \
  -exec stat -f "%z|%m|%N" {} + 2>/dev/null' |
awk -F'|' '
function ext(p,l) { l=tolower(p); sub(/^.*\./,"",l); return l }
{
  e=ext($3); n[e]++; b[e]+=$1
  if (!(e in o) || $2<o[e]) o[e]=$2
  if (!(e in z) || $2>z[e]) z[e]=$2
}
END {
  for (e in n)
    printf "%s|%d|%.0f|%d|%d\n",e,n[e],b[e],o[e],z[e]
}' | sort
```

Exact selected-hash command:

```bash
ssh 100.82.140.72 '
for f in \
  "/Volumes/500GB/marklab/workspace/marklab-spatial-phenotype-recovery/docs/DATA_PROVENANCE.md" \
  "/Volumes/500GB/marklab/manifests/source-records.json" \
  "/Volumes/500GB/marklab/manifests/cellvit-source-provenance.sha256" \
  "/Volumes/500GB/marklab/raw/tcga-crc-he-validation-v1/sha256_manifest.txt" \
  "/Volumes/500GB/marklab/manifests/tcga-crc-gdc-validation-v1/physical-scale-preflight-v1/preflight_summary.json" \
  "/Volumes/500GB/marklab/derived/tcga-crc-he-cellvit-features-v1/cellvit_model.json" \
  "/Volumes/500GB/marklab/derived/schurch-he-cellvit-features-v2/cellvit_transform.npz" \
  "/Volumes/500GB/marklab/runs/tcga-he-cellvit-validation-v2-scale-qc/inference_sha256_manifest.txt" \
  "/Volumes/500GB/marklab/runs/tcga-he-spatial-full-embedding-v1/run_manifest.json" \
  "/Volumes/500GB/marklab/runs/tcga-he-spatial-full-embedding-v1/analysis_config.json" \
  "/Volumes/1TB/marklab/manifests/cptac-coad-he-v1/slide_sha256_manifest.txt" \
  "/Volumes/1TB/marklab/manifests/cptac-coad-he-v1/physical-scale-preflight-v1/preflight_summary.json" \
  "/Volumes/1TB/marklab/manifests/cptac-coad-he-v1/manifest_summary.json" \
  "/Volumes/500GB/marklab/manifests/schurch-crc-he-cellvit-2020.sha256"
do
  [ -f "$f" ] || { printf "MISSING|%s\n" "$f"; continue; }
  sz=$(stat -f "%z" "$f")
  mt=$(stat -f "%Sm" -t "%Y-%m-%dT%H:%M:%S%z" "$f")
  h=$(shasum -a 256 "$f" | cut -d " " -f1)
  printf "%s|%s|%s|%s\n" "$sz" "$mt" "$h" "$f"
done'

ssh 100.82.140.72 '
f="/Volumes/500GB/marklab/env/models/CellViT-SAM-H-x40-AMP.pth"
sz=$(stat -f "%z" "$f")
mt=$(stat -f "%Sm" -t "%Y-%m-%dT%H:%M:%S%z" "$f")
h=$(shasum -a 256 "$f" | cut -d " " -f1)
printf "%s|%s|%s|%s\n" "$sz" "$mt" "$h" "$f"'
```

Exact safe NPY-header command (identifier-valued output omitted):

```bash
ssh 100.82.140.72 '/Volumes/500GB/marklab/env/cellvit-mps-py39/bin/python3 - <<'"'"'PY'"'"'
from pathlib import Path
from collections import defaultdict
import numpy as np

roots = [
    Path("/Users/user/Bench/CellViT-plus-plus/results/public_validation"),
    Path("/Volumes/500GB/marklab"),
    Path("/Volumes/1TB/marklab"),
]
excluded = {".git", "target", "node_modules", "env", ".venv", "venv", "site-packages"}

def cohort(path):
    value = str(path)
    if "/results/public_validation/" in value:
        return value.split("/results/public_validation/", 1)[1].split("/", 1)[0]
    if "/Volumes/500GB/marklab/" in value:
        parts = value.split("/Volumes/500GB/marklab/", 1)[1].split("/")
        return "500GB/" + "/".join(parts[:2])
    if "/Volumes/1TB/marklab/" in value:
        parts = value.split("/Volumes/1TB/marklab/", 1)[1].split("/")
        return "1TB/" + "/".join(parts[:2])
    return "other"

summary = defaultdict(lambda: [0, 0, 0])
for root in roots:
    for path in root.rglob("*.npy"):
        if any(part in excluded for part in path.parts):
            continue
        if "embedding" not in path.name.lower() and "embedding" not in str(path.parent).lower():
            continue
        try:
            array = np.load(path, mmap_mode="r", allow_pickle=False)
            dimension = array.shape[-1] if array.ndim else 1
            rows = array.shape[0] if array.ndim else 1
            key = (cohort(path), str(dimension), str(array.dtype))
            summary[key][0] += 1
            summary[key][1] += rows
            summary[key][2] += path.stat().st_size
        except Exception:
            pass

for key, values in sorted(summary.items()):
    print("|".join(key), *values, sep="|")
PY'
```

Exact Snappy JSON schema command (identifier-valued branches omitted):

```bash
ssh 100.82.140.72 '/Volumes/500GB/marklab/env/cellvit-mps-py39/bin/python3 - <<'"'"'PY'"'"'
from pathlib import Path
import json
import snappy

root = Path("/Volumes/500GB/marklab/derived/tcga-crc-he-cellvit-inference-v1")
path = next(root.rglob("*_cells.json.snappy"))
document = json.loads(snappy.decompress(path.read_bytes()))

print("top_type", type(document).__name__)
print("top_keys", sorted(document))
print("wsi_metadata_keys", sorted(document["wsi_metadata"]))
print("type_map_size", len(document["type_map"]))
cells = document["cells"]
print("cell_count", len(cells))
print("cell_item_keys", sorted(cells[0]) if cells else [])
PY'
```

Exact HDF5 metadata command (representative filename omitted from output):

```bash
ssh 100.82.140.72 '/Volumes/500GB/marklab/env/cellvit-mps-py39/bin/python3 - <<'"'"'PY'"'"'
from pathlib import Path
import h5py

root = Path("/Users/user/Bench/CellViT-plus-plus/results/public_validation/hest_clonal_prostate/hest/patches")
path = next(root.glob("*.h5"))

with h5py.File(path, "r") as handle:
    print("root_attrs", {key: str(value) for key, value in handle.attrs.items()})

    def inspect(name, obj):
        if isinstance(obj, h5py.Dataset):
            print(
                name,
                obj.shape,
                str(obj.dtype),
                {key: str(value) for key, value in obj.attrs.items()},
            )

    handle.visititems(inspect)
PY'
```

No `.pt` file was opened.

Final local commands:

```bash
git status --short
git diff --check
trash /tmp/marklab-slide-inventory.FohXVF
```

## Tests not run

- Full WSI rehash: intentionally not run; 198 GB of SVS data already has per-file manifests and the task was bounded to inventory/provenance.
- `.pt` deserialization: prohibited for untrusted pickle; requires a trusted converter task.
- Marklab ingestion/scientific tests: not applicable; no importer exists yet.
- OME-NGFF/Zarr validation: no corpus found.

## Numerical/scientific evidence

- Oracle: remote SHA-256 manifests and independently rehashed selected checkpoint/manifests.
- Tolerance: exact hashes/byte counts for selected evidence; aggregate inventory counts exact for scanned roots.
- Simulation/calibration: not applicable.
- Real-data validation: assets found, not analyzed by a new Marklab method.
- Determinism: manifests describe deterministic sampling and content digests; no new computation was validated.
- Undefined-state coverage: missing scale metadata, missing stable `CellId`, missing patch vectors/links, and missing observation masks recorded explicitly.

## Performance evidence

No performance claim. The full checkpoint hash was feasible; full WSI rehash was deliberately avoided. Inventory was output-size dependent over authorized roots.

## API/config/CLI/schema/artifact changes

None.

## Assumptions

1. Existing authenticated SSH scope and the named Marklab/public-validation roots were user-authorized.
2. A >100 MiB TIFF is WSI-compatible evidence, not necessarily a unique raw biological slide.

## Decisions

- DEC-0007: interpret the authorized “slides” as pathology WSI/data and require safe digest-referenced ingestion.

## Known risks

1. Cell rows lack a universal explicit `CellId`; positional correspondence can drift.
2. `.pt` serialization is executable pickle.
3. Full sampled-tissue masks were not persisted for the main CellViT inference trees.
4. Patch embeddings/links were not found.
5. Nine selected TCGA slides lack usable physical scale; seven advertised CPTAC slides are not accessible.
6. Patient/sample identifiers remain in access-controlled paths/manifests and must not enter repository logs.

## Remaining work

1. Design and validate a trusted `.pt`/JSON/NPY-to-Arrow/Parquet converter with deterministic `CellId` alignment.
2. Materialize explicit sampled-patch observation windows.
3. Locate or generate canonical patch embeddings and `CellPatchLink` artifacts.

## Next exact action

Complete WS-B, then freeze C-01/C-03/C-04 contracts against these manifests before reading patient-level content.

## Scope audit

- Unrelated files changed: no.
- Dependency changes: no.
- Formatting sweep: no.
- Hidden result-format change: no.
- Duplicate canonical implementation introduced: no.
- Unsupported claim introduced: no.
- Worktree status: repository content unchanged by this task; lead-owned WS-A documentation remained the only dirty scope.
