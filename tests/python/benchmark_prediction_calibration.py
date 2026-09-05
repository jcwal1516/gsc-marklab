"""Frozen calibration oracles, profiling and correctness-checked paired measurements.

Use the locked Python environment and OMP/OPENBLAS/MKL_NUM_THREADS=1.
--prepare deliberately regenerates the declared fixtures; comparison never rewrites them.
"""
import importlib.util, json, hashlib, math, pathlib, cProfile, pstats
root = pathlib.Path(__file__).resolve().parents[2]
worker = root / 'workers/python/marklab_scipy_prediction_calibration_worker.py'
s = importlib.util.spec_from_file_location('oracle', worker)
m = importlib.util.module_from_spec(s)
s.loader.exec_module(m)
out = root / 'crates/marklab-bayes/tests/fixtures/prediction_calibration'

def prepare():
    for name, n in [('small', 16), ('representative', 1000), ('demanding', 10000), ('constant', 16), ('separated', 32), ('saturated', 16), ('representative_binary_scores', 1000), ('demanding_binary_scores', 10000)]:
        rows = []
        for i in range(n):
            if name in ('small', 'saturated'):
                scores = [-2, -1.5, -1, -0.5, 0.5, 1, 1.5, 2, -1.75, -1.25, -0.75, -0.25, 0.25, 0.75, 1.25, 1.75]
                labels = [0, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 1]
                x = scores[i]
                y = labels[i]
                if name == 'saturated':
                    x = x / 10 if i < 8 else 1.7976931348623157e+308 * (1 if x > 0 else -1)
            elif name == 'constant':
                x = 0.0
                y = i % 2
            elif name == 'separated':
                x = float(i % 16 - 8)
                y = int(x >= 0)
            else:
                x = 3 * math.sin(i * 0.317) + math.cos(i * 0.791)
                y = int(x + 2 * math.sin(i * 1.733) > 0)
            if name.endswith('_binary_scores'):
                x = round(x * 8) / 8
            rows.append(dict(patient_id=f'p{i:06}', split='training_oof' if i < n // 2 else 'test', score=x, label=y))
        spec = dict(rows=rows, bins=4, timeout_seconds=120)
        req = dict(format='marklab.scipy_prediction_calibration_request', version=1, method='platt_logistic', backend=dict(name='scipy', version='1.18.1', python_version='3.12', environment_lock_sha256=hashlib.sha256(worker.with_name('uv.lock').read_bytes()).hexdigest(), worker_sha256=hashlib.sha256(worker.read_bytes()).hexdigest()), rows=rows, bins=4, resources=dict(maximum_patients=100000, maximum_bins=20, maximum_output_bytes=16777216, timeout_seconds=120))
        raw = json.dumps(req, separators=(',', ':')).encode()
        prof = cProfile.Profile()
        try:
            result = prof.runcall(m.run, raw)
        except Exception as e:
            print(name, 'FAILED', e)
            continue
        for suffix, obj in [('spec', spec), ('oracle', result), ('request', req)]:
            (out / f'{name}.{suffix}.json').write_text(json.dumps(obj, separators=(',', ':')) + '\n')
        with open(root / f'target/native-migration/calibration/{name}.profile.txt', 'w') as f:
            pstats.Stats(prof, stream=f).sort_stats('cumulative').print_stats(20)
        print(name, result['calibrator'], result['metrics']['calibration_slope'])
import argparse, os, platform, random, statistics, subprocess, tempfile, time

def verify(reference, native):

    def compare(a, b, path):
        if isinstance(a, dict):
            assert set(a) == set(b), (path, set(a), set(b))
            for key in a:
                compare(a[key], b[key], path + '.' + key)
        elif isinstance(a, list):
            assert len(a) == len(b), (path, len(a), len(b))
            for i, (x, y) in enumerate(zip(a, b)):
                compare(x, y, f'{path}[{i}]')
        elif isinstance(a, float):
            if path.endswith((".score", ".lower", ".upper")):
                assert a.hex() == float(b).hex(), (path, a, b)
                return
            tolerance = 1e-12 if 'wilson' in path else 1e-06 * (1 + abs(a)) if any((k in path for k in ['slope', 'intercept', 'calibration_in_the_large'])) else 2e-07
            assert math.isfinite(b) and abs(a - b) <= tolerance, (path, a, b, tolerance)
        else:
            assert a == b, (path, a, b)
    for key in ['calibrator', 'predictions', 'metrics']:
        compare(reference[key], native[key], key)

def summary(samples):
    ratios = [s['python'] / s['rust'] for s in samples]
    rng = random.Random(20260905)
    bootstrap = sorted((statistics.median(rng.choices(ratios, k=len(ratios))) for _ in range(10000)))
    return dict(samples_ns=samples, python_median_ns=statistics.median((s['python'] for s in samples)), rust_median_ns=statistics.median((s['rust'] for s in samples)), median_paired_speedup=statistics.median(ratios), paired_bootstrap_95pct=[bootstrap[250], bootstrap[9749]], parity=True, promotable=bootstrap[250] > 1)

def measure(warm, baseline, candidate):
    directory = root / 'target/native-migration/calibration'
    report = dict(platform=platform.platform(), python=platform.python_version(), threads=1, workloads={}, identities={})
    for label, path in [('worker', worker), ('lock', worker.with_name('uv.lock')), ('baseline', baseline), ('candidate', candidate), ('warm', warm)]:
        report['identities'][label] = dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    env = os.environ.copy()
    env.update(MARKLAB_RUNTIME_ROOT=str(root), MARKLAB_PYTHON=str(root / 'target/pymc-venv/bin/python'), OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1')
    env.pop('MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION', None)
    for name in ['small', 'representative', 'demanding', 'constant', 'separated', 'saturated', 'representative_binary_scores', 'demanding_binary_scores']:
        spec_raw = (out / f'{name}.spec.json').read_bytes()
        raw = (out / f'{name}.request.json').read_bytes()
        spec = json.loads(spec_raw)
        oracle = json.loads((out / f'{name}.oracle.json').read_bytes())
        result = dict(request_sha256=hashlib.sha256(raw).hexdigest(), spec_sha256=hashlib.sha256(spec_raw).hexdigest())
        report['workloads'][name] = result
        for phase in ['warm', 'cold']:
            samples = []
            try:
                m.run(raw)
                with tempfile.TemporaryDirectory(prefix='marklab-calibration-') as temp:
                    input_path = pathlib.Path(temp) / 'input.csv'
                    output_path = pathlib.Path(temp) / 'result.json'
                    input_path.write_text('patient_id,split,score,label\n' + ''.join((f"{r['patient_id']},{r['split']},{r['score']!r},{r['label']}\n" for r in spec['rows'])))
                    native_wire = json.dumps(dict(csv=input_path.read_text(), bins=4, timeout_seconds=120), separators=(',', ':')).encode()
                    for repeat in range(10):
                        pair = {}
                        for label in ['python', 'rust'] if repeat % 2 == 0 else ['rust', 'python']:
                            if phase == 'warm':
                                if label == 'python':
                                    start = time.perf_counter_ns()
                                    value = m.run(raw)
                                    json.dumps(value, separators=(',', ':'), allow_nan=False).encode()
                                    elapsed = time.perf_counter_ns() - start
                                else:
                                    packet = json.loads(subprocess.run([str(warm), '2'], input=native_wire, capture_output=True, check=True, env=env, timeout=120).stdout)
                                    value = packet['result']
                                    elapsed = packet['nanoseconds'][1]
                            else:
                                destination = output_path.with_name(f'{repeat}-{label}.json')
                                binary = baseline if label == 'python' else candidate
                                start = time.perf_counter_ns()
                                subprocess.run([str(binary), 'bayes', 'calibrate-predictions', '--method', 'platt-logistic', '--input', str(input_path), '--bins', '4', '--timeout-seconds', '120', '--out', str(destination)], capture_output=True, check=True, env=env, timeout=125)
                                elapsed = time.perf_counter_ns() - start
                                value = json.loads(destination.read_bytes())
                            verify(oracle, value)
                            if label == "rust":
                                result["native_backend"] = value["backend"]
                                result["native_request_sha256"] = value["request_sha256"]
                            pair[label] = elapsed
                        samples.append(pair)
                    if phase == 'cold':
                        memory = {}
                        for label, binary in [('python', baseline), ('rust', candidate)]:
                            destination = output_path.with_name(f'rss-{label}.json')
                            run = subprocess.run(['/usr/bin/time', '-l', str(binary), 'bayes', 'calibrate-predictions', '--method', 'platt-logistic', '--input', str(input_path), '--bins', '4', '--timeout-seconds', '120', '--out', str(destination)], capture_output=True, check=True, env=env, timeout=125)
                            verify(oracle, json.loads(destination.read_bytes()))
                            memory[label] = run.stderr.decode()
                        result['rss_time_l'] = memory
                result[phase] = summary(samples)
                print(name, phase, result[phase]['median_paired_speedup'], result[phase]['paired_bootstrap_95pct'], flush=True)
            except Exception as error:
                result[phase] = dict(samples_ns=samples, failure=str(error), stderr=(getattr(error, 'stderr', None) or b'').decode(errors='replace'), promotable=False)
                print(name, phase, 'FAILED', error, flush=True)
        (directory / 'comparison.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--warm', type=pathlib.Path)
    parser.add_argument('--baseline', type=pathlib.Path)
    parser.add_argument('--candidate', type=pathlib.Path)
    args = parser.parse_args()
    if args.prepare:
        prepare()
    if args.warm and args.baseline and args.candidate:
        measure(args.warm.resolve(), args.baseline.resolve(), args.candidate.resolve())
