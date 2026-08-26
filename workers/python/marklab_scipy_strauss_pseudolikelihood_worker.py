#!/usr/bin/env python3
"""Static SciPy worker for nested-grid Strauss pseudolikelihood."""
from __future__ import annotations
import hashlib, json, math, sys
from pathlib import Path
from typing import Any
import numpy as np
from scipy import optimize
import scipy

VERSION = "1.18.1"
class ContractError(ValueError): pass

def obj(v: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(v, dict) or set(v) != keys:
        actual = set(v) if isinstance(v, dict) else set()
        raise ContractError(f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}")
    return v
def num(v: Any, path: str) -> float:
    if isinstance(v, bool) or not isinstance(v, (int,float)) or not math.isfinite(float(v)): raise ContractError(f"{path} numeric finite")
    return float(v)
def integer(v: Any, path: str, lo: int, hi: int) -> int:
    if isinstance(v,bool) or not isinstance(v,int) or not lo <= v <= hi: raise ContractError(f"{path} integer range")
    return v
def exact(v: Any, expected: Any, path: str) -> None:
    if v != expected: raise ContractError(f"{path} mismatch")

def resolution(raw: Any, path: str) -> dict[str, Any]:
    r = obj(raw, {"grid_x","grid_y","cell_area_um2","weight_sum_um2","observed_nodes","dummy_nodes","neighbor_visits","rows"}, path)
    gx, gy = integer(r["grid_x"], path+".gx",1,100000), integer(r["grid_y"],path+".gy",1,100000)
    area, weight_sum = num(r["cell_area_um2"],path+".area"), num(r["weight_sum_um2"],path+".sum")
    observed, dummy = integer(r["observed_nodes"],path+".observed",1,10000), integer(r["dummy_nodes"],path+".dummy",1,100000)
    rows = r["rows"]
    if not isinstance(rows,list) or len(rows) != observed+dummy or dummy != gx*gy: raise ContractError(path+" dimensions")
    response=[]; weight=[]; neighbors=[]; blocks=[]
    for i, rawrow in enumerate(rows):
        row=obj(rawrow,{"node_id","node_kind","x_um","y_um","weight_um2","response","neighbor_count","block"},f"{path}.rows[{i}]")
        if not isinstance(row["node_id"],str) or not row["node_id"] or row["node_kind"] not in {"observed","dummy"}: raise ContractError("node identity")
        num(row["x_um"],"x"); num(row["y_um"],"y")
        w,y=num(row["weight_um2"],"weight"),num(row["response"],"response")
        n,b=integer(row["neighbor_count"],"neighbors",0,10000),integer(row["block"],"block",0,3)
        if w<=0 or y<0 or (row["node_kind"]=="observed") != (y>0): raise ContractError("node values")
        response.append(y); weight.append(w); neighbors.append(n); blocks.append(b)
    if abs(sum(weight)-weight_sum) > 1e-9*max(1,weight_sum): raise ContractError("weight sum")
    integer(r["neighbor_visits"],path+".visits",1,100000000)
    return {"gx":gx,"gy":gy,"area":area,"weight_sum":weight_sum,"y":np.array(response),"w":np.array(weight),"z":np.array(neighbors,dtype=float),"block":np.array(blocks)}

def validate(request: Any, lock: str, worker: str) -> dict[str, Any]:
    q=obj(request,{"format","version","backend","model","window","interaction_radius_um","bounds","maximum_iterations","coarse","fine","resources"},"request")
    exact(q["format"],"marklab.pymc_worker_request","format"); integer(q["version"],"version",1,1)
    b=obj(q["backend"],{"name","version","python_version","environment_lock_sha256","worker_sha256"},"backend")
    exact(b["name"],"scipy","backend"); exact(b["version"],"scipy-1.18.1","version"); exact(b["python_version"],"3.12","python"); exact(b["environment_lock_sha256"],lock,"lock"); exact(b["worker_sha256"],worker,"worker")
    m=obj(q["model"],{"family","conditional_intensity","quadrature","pair_boundary","uncertainty","backend_capability","maturity"},"model")
    exact(m["family"],"strauss_inhibitory_point_process","family"); exact(m["backend_capability"],"bounded_weighted_poisson_optimization","capability")
    bounds=obj(q["bounds"],{"beta_min_per_um2","beta_max_per_um2","gamma_min","gamma_max"},"bounds")
    lower=np.log([num(bounds["beta_min_per_um2"],"bmin"),num(bounds["gamma_min"],"gmin")]); upper=np.log([num(bounds["beta_max_per_um2"],"bmax"),num(bounds["gamma_max"],"gmax")])
    if np.any(lower>=upper) or upper[1]>1e-15: raise ContractError("bounds")
    iterations=integer(q["maximum_iterations"],"iterations",10,100000)
    resources=obj(q["resources"],{"maximum_table_rows","maximum_iterations","maximum_neighbor_visits","maximum_output_bytes","timeout_seconds"},"resources")
    integer(resources["maximum_table_rows"],"rows",1,100000); integer(resources["maximum_iterations"],"iters",10,100000); integer(resources["maximum_neighbor_visits"],"visits",1,100000000); integer(resources["maximum_output_bytes"],"output",1,1048576); integer(resources["timeout_seconds"],"timeout",1,3600)
    return {"lower":lower,"upper":upper,"iterations":iterations,"coarse":resolution(q["coarse"],"coarse"),"fine":resolution(q["fine"],"fine")}

def fit_one(r: dict[str,Any], lower: np.ndarray, upper: np.ndarray, iterations: int) -> dict[str,Any]:
    X=np.column_stack([np.ones(r["y"].size),r["z"]]); w,y=r["w"],r["y"]
    def fun(t):
        eta=X@t; return float(np.sum(w*(np.exp(eta)-y*eta)))
    def jac(t):
        eta=X@t; return X.T@(w*(np.exp(eta)-y))
    initial=(lower+upper)/2; initial_obj=fun(initial)
    result=optimize.minimize(fun,initial,jac=jac,bounds=list(zip(lower,upper)),method="L-BFGS-B",options={"maxiter":iterations,"gtol":1e-12,"ftol":1e-15,"maxls":50})
    theta=np.asarray(result.x); eta=X@theta; mu=np.exp(eta); hessian=X.T@(X*(w*mu)[:,None]); covariance=np.linalg.inv(hessian)
    scores=np.array([np.sum((w*(mu-y))[:,None]*X*(r["block"]==block)[:,None],axis=0) for block in range(4)])
    robust=covariance@(scores.T@scores)@covariance
    gradient=jac(theta); projected=gradient.copy()
    for i in range(2):
        if theta[i] <= lower[i]+1e-10 and gradient[i]>0: projected[i]=0
        if theta[i] >= upper[i]-1e-10 and gradient[i]<0: projected[i]=0
    return {"grid_x":r["gx"],"grid_y":r["gy"],"beta_per_um2":float(np.exp(theta[0])),"gamma":float(np.exp(theta[1])),"log_beta":float(theta[0]),"log_gamma":float(theta[1]),"objective":fun(theta),"initial_objective":initial_obj,"gradient_norm":float(np.linalg.norm(projected,np.inf)),"hessian_condition":float(np.linalg.cond(hessian)),"model_se_log_beta":float(np.sqrt(covariance[0,0])),"model_se_log_gamma":float(np.sqrt(covariance[1,1])),"robust_se_log_beta":float(np.sqrt(max(robust[0,0],np.finfo(float).tiny))),"robust_se_log_gamma":float(np.sqrt(max(robust[1,1],np.finfo(float).tiny))),"weight_sum_um2":r["weight_sum"],"converged":bool(result.success),"evaluations":int(result.nfev),"message":str(result.message)}

def main() -> None:
    if scipy.__version__ != VERSION or sys.version_info[:2] != (3,12): raise ContractError("version drift")
    script=Path(__file__); lock=hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest(); worker=hashlib.sha256(script.read_bytes()).hexdigest(); raw=sys.stdin.buffer.read(16*1024*1024+1)
    if len(raw)>16*1024*1024: raise ContractError("request too large")
    request_sha=hashlib.sha256(raw).hexdigest(); c=validate(json.loads(raw),lock,worker); coarse=fit_one(c["coarse"],c["lower"],c["upper"],c["iterations"]); fine=fit_one(c["fine"],c["lower"],c["upper"],c["iterations"])
    complete=all(f["converged"] and f["gradient_norm"]<=1e-6 and all(math.isfinite(float(f[k])) for k in ["beta_per_um2","gamma","objective","model_se_log_beta","model_se_log_gamma","robust_se_log_beta","robust_se_log_gamma"]) for f in [coarse,fine])
    out={"format":"marklab.scipy_strauss_pseudolikelihood_worker_result","version":1,"backend":{"name":"scipy","version":"scipy-1.18.1","python_version":"3.12","environment_lock_sha256":lock,"worker_sha256":worker},"request_sha256":request_sha,"fit_state":"complete" if complete else "nonconverged","coarse_fit":coarse,"fine_fit":fine,"refinement":{"absolute_beta_difference":abs(coarse["beta_per_um2"]-fine["beta_per_um2"]),"absolute_gamma_difference":abs(coarse["gamma"]-fine["gamma"]),"absolute_objective_difference":abs(coarse["objective"]-fine["objective"])}}
    sys.stdout.write(json.dumps(out,allow_nan=False,sort_keys=True,separators=(",",":"))+"\n")
if __name__ == "__main__":
    try: main()
    except Exception as e:
        print(f"marklab SciPy Strauss pseudolikelihood worker failed: {type(e).__name__}: {e}",file=sys.stderr); raise SystemExit(2) from e
