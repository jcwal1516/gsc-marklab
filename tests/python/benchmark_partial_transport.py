"""Frozen entropic partial-transport inputs and bounded independent Python references."""
import argparse
import cProfile
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import pstats
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[2]
WORKER=ROOT/'workers/python/marklab_scipy_partial_transport_worker.py'
FIXTURES=ROOT/'crates/marklab-bayes/tests/fixtures/partial_transport'
OUTPUT=ROOT/'target/native-migration/partial-transport'


def cases():
    base=[('forced',[2.],[3.],[4.],1.5,.5),('inactive',[2.,2.],[2.,2.,2.],[0.,.5,1.,1.,.5,0.],1.,.5),('binding',[.25,1.,2.],[.5,1.,2.],[0.,.25,1.,.25,0.,.75,1.,.75,0.],2.,.3),('zero_capacity',[0.,1.,2.],[1.,0.,2.],[0.,.25,1.,.25,0.,.75,1.,.75,0.],1.5,.5),('balanced',[1.,2.],[2.,1.],[0.,1.,1.,0.],3.,.5)]
    for name,n in [('representative',8),('demanding',16),('maximum_null',64)]:
        a=[1.+(i%3)/4 for i in range(n)];b=[.75+(j%4)/4 for j in range(n)]
        cost=[(i-j)**2/(2*n)+(i*j%5)/16 for i in range(n) for j in range(n)]
        mass=.75*min(sum(a),sum(b));epsilon=.75
        if name=='maximum_null':a=b=[1.]*n;cost=[1.]*(n*n);mass=32.;epsilon=.5
        base.append((name,a,b,cost,mass,epsilon))
    for name,a,b,cost,mass,epsilon in base:
        yield name,dict(source=[dict(id=f's{i:03}',mass=x) for i,x in enumerate(a)],target=[dict(id=f't{j:03}',mass=x) for j,x in enumerate(b)],costs_row_major=cost,transported_mass=mass,epsilon=epsilon,timeout_seconds=120)


def request(spec):
    return dict(format='marklab.scipy_partial_transport_request',version=1,backend=dict(name='scipy',version='1.18.1',python_version='3.12',environment_lock_sha256=hashlib.sha256(WORKER.with_name('uv.lock').read_bytes()).hexdigest(),worker_sha256=hashlib.sha256(WORKER.read_bytes()).hexdigest()),**{k:v for k,v in spec.items() if k!='timeout_seconds'},resources=dict(maximum_source_points=64,maximum_target_points=64,maximum_plan_variables=4096,feasibility_tolerance=1e-8,maximum_output_bytes=16777216,timeout_seconds=120))


def prepare():
    FIXTURES.mkdir(parents=True,exist_ok=True);OUTPUT.mkdir(parents=True,exist_ok=True)
    for name,spec in cases():
        raw=json.dumps(request(spec),separators=(',',':'),allow_nan=False).encode()
        (FIXTURES/f'{name}.request.json').write_bytes(raw)
        (FIXTURES/f'{name}.spec.json').write_text(json.dumps(spec,separators=(',',':'),allow_nan=False)+'\n')
        try:
            result=subprocess.run([sys.executable,str(WORKER)],input=raw,capture_output=True,check=True,timeout=125)
            value=json.loads(result.stdout)
            assert value['request_sha256']==hashlib.sha256(raw).hexdigest()
            (FIXTURES/f'{name}.oracle.json').write_bytes(result.stdout)
            print(name,'reference passed',value['optimizer']['iterations'],flush=True)
        except Exception as error:
            failure=dict(error=str(error),stderr=(getattr(error,'stderr',None) or b'').decode(errors='replace'))
            (OUTPUT/f'{name}.failure.json').write_text(json.dumps(failure,indent=2)+'\n')
            print(name,'REFERENCE FAILED',failure,flush=True)
    module=importlib.util.spec_from_file_location('partial_reference',WORKER);worker=importlib.util.module_from_spec(module);module.loader.exec_module(worker)
    for name in ['forced','representative']:
        if not (FIXTURES/f'{name}.oracle.json').exists():continue
        profile=cProfile.Profile();profile.runcall(worker.run,(FIXTURES/f'{name}.request.json').read_bytes())
        with (OUTPUT/f'{name}.profile.txt').open('w') as stream:pstats.Stats(profile,stream=stream).sort_stats('cumulative').print_stats(20)

import csv
import io
import math
import platform
import tempfile
import time
from benchmark_late_fusion import summary


def csv_files(spec):
    files=[]
    for field,header in [('source','source_id'),('target','target_id')]:
        stream=io.StringIO();writer=csv.writer(stream,lineterminator='\n');writer.writerow([header,'mass'])
        for r in spec[field]:writer.writerow([r['id'],repr(r['mass'])])
        files.append(stream.getvalue())
    stream=io.StringIO();writer=csv.writer(stream,lineterminator='\n');writer.writerow(['source_id','target_id','cost'])
    n=len(spec['target'])
    for i,a in enumerate(spec['source']):
        for j,b in enumerate(spec['target']):writer.writerow([a['id'],b['id'],repr(spec['costs_row_major'][i*n+j])])
    files.append(stream.getvalue())
    return files


def verify(a,b):
    def compare(x,y,path):
        if isinstance(x,dict):
            assert set(x)==set(y),(path,set(x),set(y))
            for k,v in x.items():compare(v,y[k],path+'.'+k)
        elif isinstance(x,list):
            assert len(x)==len(y),path
            for i,(u,v) in enumerate(zip(x,y)):compare(u,v,f'{path}[{i}]')
        elif isinstance(x,float):
            assert math.isfinite(y),(path,y)
            if path.endswith('.cost'):assert x.hex()==float(y).hex(),(path,x,y)
            else:assert abs(x-y)<=1e-6*(1+abs(x)),(path,x,y)
        else:assert x==y,(path,x,y)
    for key in ['plan','source_marginals','target_marginals','transported_mass','unmatched_source_mass','unmatched_target_mass','transport_cost','entropy','regularized_objective','maximum_constraint_violation','constraint_status']:compare(a[key],b[key],key)
    assert b['maximum_constraint_violation']<=1e-8


def measure(warm,baseline,candidate):
    module=importlib.util.spec_from_file_location('partial_reference',WORKER);worker=importlib.util.module_from_spec(module);module.loader.exec_module(worker)
    report=dict(platform=platform.platform(),python=platform.python_version(),threads=1,warm_boundary='Rust CSV admission/source binding/science/serialization; Python original JSON worker/serialization; input file reads and process startup excluded',workloads={},identities={})
    for label,path in [('worker',WORKER),('lock',WORKER.with_name('uv.lock')),('baseline',baseline),('candidate',candidate),('warm',warm)]:report['identities'][label]=dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    env=os.environ.copy();env.update(MARKLAB_RUNTIME_ROOT=str(ROOT),MARKLAB_PYTHON=str(ROOT/'target/pymc-venv/bin/python'),OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1');env.pop('MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION',None)
    for name,_ in cases():
        spec_raw=(FIXTURES/f'{name}.spec.json').read_bytes();spec=json.loads(spec_raw);raw=(FIXTURES/f'{name}.request.json').read_bytes()
        result=dict(spec_sha256=hashlib.sha256(spec_raw).hexdigest(),request_sha256=hashlib.sha256(raw).hexdigest());report['workloads'][name]=result
        oracle_path=FIXTURES/f'{name}.oracle.json'
        if not oracle_path.exists():
            result.update(reference_failure=json.loads((OUTPUT/f'{name}.failure.json').read_bytes()),promotable=False)
            continue
        oracle=json.loads(oracle_path.read_bytes())
        with tempfile.TemporaryDirectory(prefix='marklab-partial-transport-') as temp:
            paths=[Path(temp)/f'{k}.csv' for k in ['source','target','cost']]
            for path,content in zip(paths,csv_files(spec)):path.write_text(content)
            controls=['--transported-mass',repr(spec['transported_mass']),'--epsilon',repr(spec['epsilon']),'--timeout-seconds','120']
            csv_args=[arg for key,path in zip(['--source','--target','--cost'],paths) for arg in (key,str(path))]
            warm_args=['2',*[str(p) for p in paths],repr(spec['transported_mass']),repr(spec['epsilon']),'120']
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
                                    packet=json.loads(subprocess.run([str(warm),*warm_args],capture_output=True,check=True,env=env,timeout=125).stdout);value=packet['result'];elapsed=packet['nanoseconds'][1]
                            else:
                                destination=Path(temp)/f'{repeat}-{label}.json';binary=baseline if label=='python' else candidate
                                start=time.perf_counter_ns();subprocess.run([str(binary),'bayes','partial-ot',*csv_args,*controls,'--out',str(destination)],capture_output=True,check=True,env=env,timeout=125);elapsed=time.perf_counter_ns()-start;value=json.loads(destination.read_bytes())
                            verify(oracle,value)
                            if label=='rust':result['native_backend']=value['backend'];result['native_request_sha256']=value['request_sha256']
                            pair[label]=elapsed
                        samples.append(pair)
                    result[phase]=summary(samples)
                    print(name,phase,result[phase]['median_paired_speedup'],result[phase]['paired_bootstrap_95pct'],flush=True)
                    if phase=='cold':
                        memory={}
                        for label,binary in [('python',baseline),('rust',candidate)]:
                            destination=Path(temp)/f'rss-{label}.json'
                            run=subprocess.run(['/usr/bin/time','-l',str(binary),'bayes','partial-ot',*csv_args,*controls,'--out',str(destination)],capture_output=True,check=True,env=env,timeout=125)
                            verify(oracle,json.loads(destination.read_bytes()));memory[label]=run.stderr.decode()
                        result['rss_time_l']=memory
                except Exception as error:
                    result[phase]=dict(samples_ns=samples,failure=str(error),stderr=(getattr(error,'stderr',None) or b'').decode(errors='replace'),promotable=False)
                    print(name,phase,'FAILED',error,flush=True)
                (OUTPUT/'comparison.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    (OUTPUT/'comparison.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    return all(value.get(phase,{}).get('promotable',False) for value in report['workloads'].values() if 'reference_failure' not in value for phase in ['warm','cold'])


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--prepare',action='store_true');parser.add_argument('--warm',type=Path);parser.add_argument('--baseline',type=Path);parser.add_argument('--candidate',type=Path);args=parser.parse_args()
    if args.prepare:prepare()
    if args.warm and args.baseline and args.candidate:
        if not measure(args.warm.resolve(),args.baseline.resolve(),args.candidate.resolve()):sys.exit(1)
