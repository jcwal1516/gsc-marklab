"""Declared late-fusion fixtures and independent frozen Python profiling."""
import cProfile
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import pstats
ROOT = Path(__file__).resolve().parents[2]
WORKER = ROOT / 'workers/python/marklab_scipy_late_fusion_worker.py'
FIXTURES = ROOT / 'crates/marklab-bayes/tests/fixtures/late_fusion'
OUTPUT = ROOT / 'target/native-migration/late-fusion'

def fixtures():
    for name, n, d in [('small', 30, 2), ('representative', 300, 4), ('demanding', 3000, 16), ('demanding_null', 3000, 16), ('missing_column', 300, 4), ('constant_calibration', 30, 2)]:
        patients = []
        for i in range(n):
            split = 'meta_train' if i < n // 3 else 'calibration' if i < 2 * n // 3 else 'test'
            if name in ('small', 'constant_calibration'):
                label = i % 2
                a = None if i >= 20 and i % 5 == 0 else 0.7 + 0.01 * (i % 3) if label else 0.3 - 0.01 * (i % 3)
                b = None if i >= 20 and i % 5 == 1 else 0.65 + 0.01 * (i % 4) if label else 0.35 - 0.01 * (i % 4)
                values = [a, b]
            else:
                index = i // 2 if name == 'demanding_null' else i
                latent = math.sin((index + 1) * 0.317)
                label = i % 2 if name == 'demanding_null' else int(latent + 0.6 * math.sin(i * 1.933) > 0)
                values = [None if j > 0 and ((index + j) % 7 == 0 or (name == 'missing_column' and j == d - 1)) else 0.5 + 0.45 * math.sin((index + 1) * 0.317 + j * 0.1) for j in range(d)]
            if name == 'constant_calibration' and split == 'calibration':
                values = [0.5] * d
                label = int(i - n // 3 < 2)
            patients.append(dict(patient_id=f'p{i:06}', split=split, base_prediction_source='patient_level_out_of_fold', label=label, modality_probabilities=values))
        yield (name, dict(patients=patients, modalities=[f'modality_{j}' for j in range(d)], l2_penalty=0.1, timeout_seconds=120))

def prepare():
    FIXTURES.mkdir(parents=True, exist_ok=True)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    module = importlib.util.spec_from_file_location('fusion_reference', WORKER)
    worker = importlib.util.module_from_spec(module)
    module.loader.exec_module(worker)
    for name, spec in fixtures():
        request = dict(format='marklab.scipy_late_fusion_request', version=1, backend=dict(name='scipy', version='1.18.1', python_version='3.12', environment_lock_sha256=hashlib.sha256(WORKER.with_name('uv.lock').read_bytes()).hexdigest(), worker_sha256=hashlib.sha256(WORKER.read_bytes()).hexdigest()), base_prediction_source='patient_level_out_of_fold', **{k: v for k, v in spec.items() if k != 'timeout_seconds'}, resources=dict(maximum_patients=100000, maximum_modalities=16, maximum_output_bytes=16777216, timeout_seconds=120))
        raw = json.dumps(request, separators=(',', ':'), allow_nan=False).encode()
        (FIXTURES / f'{name}.request.json').write_bytes(raw)
        (FIXTURES / f'{name}.spec.json').write_text(json.dumps(spec, separators=(',', ':'), allow_nan=False) + '\n')
        profile = cProfile.Profile()
        try:
            result = profile.runcall(worker.run, raw)
            (FIXTURES / f'{name}.oracle.json').write_text(json.dumps(result, separators=(',', ':'), allow_nan=False) + '\n')
            print(name, 'reference passed', flush=True)
        except Exception as error:
            (OUTPUT / f'{name}.failure.txt').write_text(f'{type(error).__name__}: {error}\n')
            print(name, 'REFERENCE FAILED', error, flush=True)
        with (OUTPUT / f'{name}.profile.txt').open('w') as stream:
            pstats.Stats(profile, stream=stream).sort_stats('cumulative').print_stats(20)
import argparse, csv, io, os, platform, random, statistics, struct, subprocess, tempfile, time

def reference():
    module = importlib.util.spec_from_file_location('fusion_reference', WORKER)
    instance = importlib.util.module_from_spec(module)
    module.loader.exec_module(instance)
    return instance

def csv_text(spec):
    stream = io.StringIO()
    writer = csv.writer(stream, lineterminator='\n')
    writer.writerow(['patient_id', 'split', 'base_prediction_source', 'label', *spec['modalities']])
    for p in spec['patients']:
        writer.writerow([p['patient_id'], p['split'], p['base_prediction_source'], p['label'], *[repr(v) if v is not None else '' for v in p['modality_probabilities']]])
    return stream.getvalue()

def native_wire(spec):
    return json.dumps(dict(csv=csv_text(spec), l2_penalty_bits=int.from_bytes(struct.pack('<d', spec['l2_penalty']), 'little'), timeout_seconds=spec['timeout_seconds']), separators=(',', ':')).encode()

def verify(a, b):

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
            tolerance = 1e-06 * (1 + abs(a)) if any((x in path for x in ['coefficients', 'intercept', 'slope'])) else 2e-07
            assert math.isfinite(b) and abs(a - b) <= tolerance, (path, a, b, tolerance)
        else:
            assert a == b, (path, a, b)
    for key in ['model', 'calibrator', 'predictions', 'metrics', 'missing_scenarios', 'modality_ablations']:
        compare(a[key], b[key], key)

def summary(samples):
    ratios = [s['python'] / s['rust'] for s in samples]
    rng = random.Random(20260905)
    bootstrap = sorted((statistics.median(rng.choices(ratios, k=len(ratios))) for _ in range(10000)))
    return dict(samples_ns=samples, python_median_ns=statistics.median((s['python'] for s in samples)), rust_median_ns=statistics.median((s['rust'] for s in samples)), median_paired_speedup=statistics.median(ratios), paired_bootstrap_95pct=[bootstrap[250], bootstrap[9749]], parity=True, promotable=bootstrap[250] > 1)

def measure(warm, baseline, candidate):
    m = reference()
    directory = OUTPUT
    report = dict(platform=platform.platform(), python=platform.python_version(), threads=1, workloads={}, identities={})
    for label, path in [('worker', WORKER), ('lock', WORKER.with_name('uv.lock')), ('baseline', baseline), ('candidate', candidate), ('warm', warm)]:
        report['identities'][label] = dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    env = os.environ.copy()
    env.update(MARKLAB_RUNTIME_ROOT=str(ROOT), MARKLAB_PYTHON=str(ROOT / 'target/pymc-venv/bin/python'), OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1')
    env.pop('MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION', None)
    for name in ['small', 'representative', 'demanding', 'demanding_null', 'missing_column', 'constant_calibration']:
        spec_raw = (FIXTURES / f'{name}.spec.json').read_bytes()
        raw = (FIXTURES / f'{name}.request.json').read_bytes()
        spec = json.loads(spec_raw)
        oracle_path = FIXTURES / f'{name}.oracle.json'
        if not oracle_path.exists():
            report['workloads'][name] = dict(reference_failure=(OUTPUT / f'{name}.failure.txt').read_text(), promotable=False)
            continue
        oracle = json.loads(oracle_path.read_bytes())
        result = dict(request_sha256=hashlib.sha256(raw).hexdigest(), spec_sha256=hashlib.sha256(spec_raw).hexdigest())
        report['workloads'][name] = result
        for phase in ['warm', 'cold']:
            samples = []
            try:
                m.run(raw)
                with tempfile.TemporaryDirectory(prefix='marklab-late-fusion-') as temp:
                    input_path = Path(temp) / 'input.csv'
                    output_path = Path(temp) / 'result.json'
                    input_path.write_text(csv_text(spec))
                    wire = native_wire(spec)
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
                                    packet = json.loads(subprocess.run([str(warm), '2'], input=wire, capture_output=True, check=True, env=env, timeout=120).stdout)
                                    value = packet['result']
                                    elapsed = packet['nanoseconds'][1]
                            else:
                                destination = output_path.with_name(f'{repeat}-{label}.json')
                                binary = baseline if label == 'python' else candidate
                                start = time.perf_counter_ns()
                                subprocess.run([str(binary), 'bayes', 'late-fusion', '--l2-penalty', '0.1', '--input', str(input_path), '--timeout-seconds', '120', '--out', str(destination)], capture_output=True, check=True, env=env, timeout=125)
                                elapsed = time.perf_counter_ns() - start
                                value = json.loads(destination.read_bytes())
                            verify(oracle, value)
                            if label == 'rust':
                                result['native_backend'] = value['backend']
                                result['native_request_sha256'] = value['request_sha256']
                            pair[label] = elapsed
                        samples.append(pair)
                    if phase == 'cold':
                        memory = {}
                        for label, binary in [('python', baseline), ('rust', candidate)]:
                            destination = output_path.with_name(f'rss-{label}.json')
                            run = subprocess.run(['/usr/bin/time', '-l', str(binary), 'bayes', 'late-fusion', '--l2-penalty', '0.1', '--input', str(input_path), '--timeout-seconds', '120', '--out', str(destination)], capture_output=True, check=True, env=env, timeout=125)
                            verify(oracle, json.loads(destination.read_bytes()))
                            memory[label] = run.stderr.decode()
                        result['rss_time_l'] = memory
                result[phase] = summary(samples)
                print(name, phase, result[phase]['median_paired_speedup'], result[phase]['paired_bootstrap_95pct'], flush=True)
            except Exception as error:
                result[phase] = dict(samples_ns=samples, failure=str(error), stderr=(getattr(error, 'stderr', None) or b'').decode(errors='replace'), promotable=False)
                print(name, phase, 'FAILED', error, flush=True)
        (directory / 'comparison.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    (directory / 'comparison.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
def audit_failures(candidate):
    from types import SimpleNamespace
    import numpy as np
    report={}
    for name in ['demanding','missing_column','constant_calibration']:
        spec=json.loads((FIXTURES/f'{name}.spec.json').read_bytes())
        worker=reference();calls=[]
        try:
            completed=subprocess.run([str(candidate),'backend','native-late-fusion'],input=native_wire(spec),capture_output=True,check=True,timeout=125)
            native=json.loads(completed.stdout)
            parameters=[np.asarray([native['model']['intercept'],*native['model']['coefficients']]),np.asarray([native['calibrator']['intercept'],native['calibrator']['slope']])]
            def fixed_parameters(objective,initial,*,method,jac,options):
                assert method=='BFGS' and jac is True and options=={'maxiter':1000,'gtol':1e-8}
                p=parameters[len(calls)];value,gradient=objective(p)
                maximum=float(np.max(np.abs(gradient)))
                calls.append(dict(objective=float(value),maximum_absolute_gradient=maximum,parameters=p.tolist()))
                assert math.isfinite(value) and np.all(np.isfinite(gradient)) and maximum<=1e-8,calls[-1]
                return SimpleNamespace(success=True,x=p,message='fixed native parameters; not an independent optimization')
            worker.minimize=fixed_parameters
            fixed=worker.run((FIXTURES/f'{name}.request.json').read_bytes())
            assert len(calls)==2
            verify(fixed,native)
            report[name]=dict(reference_failure=(OUTPUT/f'{name}.failure.txt').read_text(),native_backend=native['backend'],fixed_parameter_checks=calls,downstream_parity=True,independent_fit_parity=False)
            print(name,'fixed-parameter audit passed', [x['maximum_absolute_gradient'] for x in calls],flush=True)
        except Exception as error:
            report[name]=dict(failure=str(error),stderr=(getattr(error,'stderr',None) or b'').decode(errors='replace'),fixed_parameter_checks=calls,independent_fit_parity=False)
            print(name,'AUDIT FAILED',error,flush=True)
    (OUTPUT/'failed-reference-audit.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--audit-failures', type=Path)
    parser.add_argument('--warm', type=Path)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--candidate', type=Path)
    args = parser.parse_args()
    if args.prepare:
        prepare()
    if args.audit_failures:
        audit_failures(args.audit_failures.resolve())
    if args.warm and args.baseline and args.candidate:
        measure(args.warm.resolve(), args.baseline.resolve(), args.candidate.resolve())
