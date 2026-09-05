"""Frozen complete patient-held-out stacking references, including every jackknife refit."""
import argparse
import cProfile
import hashlib
import json
import math
from pathlib import Path
import pstats
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[2]
WORKER=ROOT/'workers/python/marklab_scipy_predictive_stacking_worker.py'
FIXTURES=ROOT/'crates/marklab-bayes/tests/fixtures/predictive_stacking'
OUTPUT=ROOT/'target/native-migration/predictive-stacking'


def cases():
    for name,n,d in [('symmetric',8,2),('boundary',40,3),('representative',60,3),('demanding',200,8),('maximum',500,16),('identical',50,4),('duplicate_models',8,3),('offset',8,2),('near_duplicate',40,3)]:
        patients=[]
        for i in range(n):
            if name in ['symmetric','offset','duplicate_models']:
                values=[math.log(.8),math.log(.2)] if i<4 else [math.log(.2),math.log(.8)]
                if name=='offset':values=[v-1000 for v in values]
                if name=='duplicate_models':values=[values[0],values[0],values[1]]
            elif name=='boundary':values=[-.125,-1.-(i%3)/8,-2.-(i%5)/8]
            elif name=='identical':values=[-1.-i/64]*d
            elif name=='near_duplicate':
                first=math.log(.8 if i%2 else .2);second=math.log(.2 if i%2 else .8)
                values=[first,first+1e-7*math.sin(i),second]
            else:
                values=[-.25-abs((i%d)-j)*.375+.1*math.sin((i+1)*(j+1)) for j in range(d)]
            patients.append(dict(patient_id=f'p{i:04}',held_out_unit='patient',log_predictive_densities=values))
        yield name,dict(patients=patients,model_names=[f'model_{j}' for j in range(d)],timeout_seconds=120)


def prepare():
    FIXTURES.mkdir(parents=True,exist_ok=True);OUTPUT.mkdir(parents=True,exist_ok=True)
    for name,spec in cases():
        request=dict(format='marklab.scipy_predictive_stacking_request',version=1,backend=dict(name='scipy',version='1.18.1',python_version='3.12',environment_lock_sha256=hashlib.sha256(WORKER.with_name('uv.lock').read_bytes()).hexdigest(),worker_sha256=hashlib.sha256(WORKER.read_bytes()).hexdigest()),patients=spec['patients'],model_names=spec['model_names'],resources=dict(maximum_patients=500,maximum_models=16,maximum_jackknife_density_visits=25000000,maximum_output_bytes=16777216,timeout_seconds=120))
        raw=json.dumps(request,separators=(',',':'),allow_nan=False).encode()
        (FIXTURES/f'{name}.spec.json').write_text(json.dumps(spec,separators=(',',':'),allow_nan=False)+'\n')
        (FIXTURES/f'{name}.request.json').write_bytes(raw)
        try:
            process=subprocess.run([sys.executable,str(WORKER)],input=raw,capture_output=True,check=True,timeout=125)
            result=json.loads(process.stdout);assert result['request_sha256']==hashlib.sha256(raw).hexdigest()
            (FIXTURES/f'{name}.oracle.json').write_bytes(process.stdout)
            print(name,'reference passed',flush=True)
        except Exception as error:
            failure=dict(error=str(error),stderr=(getattr(error,'stderr',None) or b'').decode(errors='replace'))
            (OUTPUT/f'{name}.failure.json').write_text(json.dumps(failure,indent=2)+'\n')
            print(name,'REFERENCE FAILED',failure,flush=True)
    import importlib.util
    module=importlib.util.spec_from_file_location('stacking_reference',WORKER);worker=importlib.util.module_from_spec(module);module.loader.exec_module(worker)
    for name in ['symmetric','representative']:
        if not (FIXTURES/f'{name}.oracle.json').exists():continue
        profile=cProfile.Profile();profile.runcall(worker.run,(FIXTURES/f'{name}.request.json').read_bytes())
        with (OUTPUT/f'{name}.profile.txt').open('w') as stream:pstats.Stats(profile,stream=stream).sort_stats('cumulative').print_stats(20)


import csv
import importlib.util
import io
import os
import platform
import tempfile
import time
from benchmark_late_fusion import summary


def csv_text(spec):
    stream=io.StringIO();writer=csv.writer(stream,lineterminator='\n');writer.writerow(['patient_id','held_out_unit',*spec['model_names']])
    for p in spec['patients']:writer.writerow([p['patient_id'],p['held_out_unit'],*[repr(v) for v in p['log_predictive_densities']]])
    return stream.getvalue()


def native_wire(spec):
    return json.dumps(dict(csv=csv_text(spec),timeout_seconds=spec['timeout_seconds']),separators=(',',':')).encode()


def verify(a,b):
    def compare(x,y,path):
        if isinstance(x,dict):
            assert set(x)==set(y),(path,set(x),set(y))
            for k,v in x.items():compare(v,y[k],path+'.'+k)
        elif isinstance(x,list):
            assert len(x)==len(y),path
            for i,(u,v) in enumerate(zip(x,y)):compare(u,v,f'{path}[{i}]')
        elif isinstance(x,float):assert math.isfinite(y) and abs(x-y)<=1e-6*(1+abs(x)),(path,x,y)
        else:assert x==y,(path,x,y)
    for key in ['weights','objective_sum_log_predictive_density','grouped_mixture_log_predictive_density','leave_one_patient_out_sensitivity']:compare(a[key],b[key],key)


def measure(warm,baseline,candidate):
    module=importlib.util.spec_from_file_location('stacking_reference',WORKER);worker=importlib.util.module_from_spec(module);module.loader.exec_module(worker)
    report=dict(platform=platform.platform(),python=platform.python_version(),threads=1,warm_boundary='Rust CSV application and Python JSON worker; both include full fit, all patient refits and serialization',workloads={},identities={})
    for label,path in [('worker',WORKER),('lock',WORKER.with_name('uv.lock')),('baseline',baseline),('candidate',candidate),('warm',warm)]:report['identities'][label]=dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    env=os.environ.copy();env.update(MARKLAB_RUNTIME_ROOT=str(ROOT),MARKLAB_PYTHON=str(ROOT/'target/pymc-venv/bin/python'),OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1');env.pop('MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION',None)
    for name,_ in cases():
        spec_raw=(FIXTURES/f'{name}.spec.json').read_bytes();spec=json.loads(spec_raw);raw=(FIXTURES/f'{name}.request.json').read_bytes();oracle=json.loads((FIXTURES/f'{name}.oracle.json').read_bytes())
        result=dict(spec_sha256=hashlib.sha256(spec_raw).hexdigest(),request_sha256=hashlib.sha256(raw).hexdigest());report['workloads'][name]=result
        with tempfile.TemporaryDirectory(prefix='marklab-predictive-stacking-') as temp:
            input_path=Path(temp)/'input.csv';input_path.write_text(csv_text(spec));wire=native_wire(spec)
            for phase in ['warm','cold']:
                samples=[]
                try:
                    worker.run(raw)
                    for repeat in range(10):
                        pair={}
                        for label in ['python','rust'] if repeat%2==0 else ['rust','python']:
                            if phase=='warm':
                                if label=='python':
                                    start=time.perf_counter_ns();value=worker.run(raw);json.dumps(value,separators=(',',':'),allow_nan=False).encode();elapsed=time.perf_counter_ns()-start
                                else:
                                    packet=json.loads(subprocess.run([str(warm),'2'],input=wire,capture_output=True,check=True,env=env,timeout=125).stdout);value=packet['result'];elapsed=packet['nanoseconds'][1]
                            else:
                                destination=Path(temp)/f'{repeat}-{label}.json';binary=baseline if label=='python' else candidate
                                start=time.perf_counter_ns();subprocess.run([str(binary),'bayes','predictive-stacking','--input',str(input_path),'--timeout-seconds','120','--out',str(destination)],capture_output=True,check=True,env=env,timeout=125);elapsed=time.perf_counter_ns()-start;value=json.loads(destination.read_bytes())
                            verify(oracle,value)
                            if label=='rust':result['native_backend']=value['backend'];result['native_request_sha256']=value['request_sha256']
                            pair[label]=elapsed
                        samples.append(pair)
                    result[phase]=summary(samples);print(name,phase,result[phase]['median_paired_speedup'],result[phase]['paired_bootstrap_95pct'],flush=True)
                    if phase=='cold':
                        memory={}
                        for label,binary in [('python',baseline),('rust',candidate)]:
                            destination=Path(temp)/f'rss-{label}.json';run=subprocess.run(['/usr/bin/time','-l',str(binary),'bayes','predictive-stacking','--input',str(input_path),'--timeout-seconds','120','--out',str(destination)],capture_output=True,check=True,env=env,timeout=125);verify(oracle,json.loads(destination.read_bytes()));memory[label]=run.stderr.decode()
                        result['rss_time_l']=memory
                except Exception as error:
                    result[phase]=dict(samples_ns=samples,failure=str(error),stderr=(getattr(error,'stderr',None) or b'').decode(errors='replace'),promotable=False);print(name,phase,'FAILED',error,flush=True)
                (OUTPUT/'comparison.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    return all(x.get(phase,{}).get('promotable',False) for x in report['workloads'].values() for phase in ['warm','cold'])


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--prepare',action='store_true');parser.add_argument('--warm',type=Path);parser.add_argument('--baseline',type=Path);parser.add_argument('--candidate',type=Path);args=parser.parse_args()
    if args.prepare:prepare()
    if args.warm and args.baseline and args.candidate:
        if not measure(args.warm.resolve(),args.baseline.resolve(),args.candidate.resolve()):sys.exit(1)
