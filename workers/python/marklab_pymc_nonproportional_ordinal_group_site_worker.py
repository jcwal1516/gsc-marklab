#!/usr/bin/env python3
from __future__ import annotations
import hashlib,json,math,sys,unicodedata
from pathlib import Path
from typing import Any
import numpy as np
import pymc as pm
import pytensor.tensor as pt
PYMC_VERSION="6.3.0"
class ContractError(ValueError):pass
def obj(v:Any,keys:set[str],path:str)->dict[str,Any]:
 if not isinstance(v,dict)or set(v)!=keys:raise ContractError(f"{path} fields differ")
 return v
def exact(v:Any,e:Any,p:str)->None:
 if v!=e:raise ContractError(f"{p} must equal {e!r}")
def number(v:Any,p:str)->float:
 if isinstance(v,bool)or not isinstance(v,(int,float))or not math.isfinite(float(v)):raise ContractError(f"{p} invalid")
 return float(v)
def integer(v:Any,p:str,l:int,h:int)->int:
 if isinstance(v,bool)or not isinstance(v,int)or not l<=v<=h:raise ContractError(f"{p} invalid")
 return v
def name(v:Any)->bool:return isinstance(v,str)and bool(v)and len(v)<=128 and v==v.strip()and not any(unicodedata.category(c)=="Cc" for c in v)
def validate(r:Any,lock:str,worker:str)->dict[str,Any]:
 r=obj(r,{"format","version","backend","model","site_ids","patients","sampling","resources","diagnostic_policy"},"request");exact(r["format"],"marklab.pymc_worker_request","format");exact(r["version"],1,"version")
 b=obj(r["backend"],{"name","version","python_version","environment_lock_sha256","worker_sha256"},"backend");exact(b["name"],"pymc","backend.name");exact(b["version"],PYMC_VERSION,"backend.version");exact(b["python_version"],"3.12","backend.python");exact(b["environment_lock_sha256"],lock,"backend.lock");exact(b["worker_sha256"],worker,"backend.worker")
 m=obj(r["model"],{"format","version","family","reference_group","comparison_group","ordered_levels","group_cutpoint_prior","cutpoint_prior_sd","site_intercept_prior","site_intercept_sd_prior_sd","threshold_effect_definition","probability_constraint","group_probability_standardization","likelihood","observation_unit","biological_unit","backend_capability","maturity"},"model")
 expected={"format":"marklab.bayesian_model_ir","version":1,"family":"patient_nonproportional_ordinal_group_site","group_cutpoint_prior":"two_independent_ordered_normal_vectors","site_intercept_prior":"sum_zero_noncentered_normal","threshold_effect_definition":"reference_cutpoint_minus_comparison_cutpoint","probability_constraint":"each_group_cutpoint_vector_strictly_ordered","group_probability_standardization":"equal_weight_sites","likelihood":"ordered_logistic_patient_outcome","observation_unit":"one_complete_ordered_outcome_per_patient","biological_unit":"patient","backend_capability":"nuts","maturity":"experimental"}
 for k,v in expected.items():exact(m[k],v,f"model.{k}")
 ref,comp,levels,sites=m["reference_group"],m["comparison_group"],m["ordered_levels"],r["site_ids"]
 if not name(ref)or not name(comp)or ref==comp or not isinstance(levels,list)or not 2<=len(levels)<=8 or not all(name(x)for x in levels)or len(set(levels))!=len(levels)or not isinstance(sites,list)or sites!=sorted(sites)or not 8<=len(sites)<=64 or len(set(sites))!=len(sites):raise ContractError("identities invalid")
 cut_sd=number(m["cutpoint_prior_sd"],"cut_sd");site_sd=number(m["site_intercept_sd_prior_sd"],"site_sd")
 if min(cut_sd,site_sd)<=0:raise ContractError("scales invalid")
 lookup={x:i for i,x in enumerate(sites)};ids=[];groups=[];outcomes=[];site_index=[];counts=np.zeros((len(sites),2),dtype=int)
 for i,raw in enumerate(r["patients"]):
  row=obj(raw,{"patient_id","site_id","group","outcome_code"},f"patient{i}")
  if not name(row["patient_id"])or row["site_id"]not in lookup or row["group"]not in{ref,comp}:raise ContractError("patient invalid")
  ids.append(row["patient_id"]);groups.append(row["group"]);outcomes.append(integer(row["outcome_code"],"outcome",0,len(levels)-1));site_index.append(lookup[row["site_id"]]);counts[site_index[-1],int(row["group"]==comp)]+=1
 if not 32<=len(ids)<=1024 or ids!=sorted(ids)or len(set(ids))!=len(ids)or np.any(counts<2)or set(outcomes)!=set(range(len(levels))):raise ContractError("support invalid")
 s=obj(r["sampling"],{"chains","tune_per_chain","draws_per_chain","target_accept","seed"},"sampling");chains=integer(s["chains"],"chains",2,8);tune=integer(s["tune_per_chain"],"tune",100,100000);draws=integer(s["draws_per_chain"],"draws",100,100000);target=number(s["target_accept"],"target");seed=integer(s["seed"],"seed",0,2**64-1)
 z=obj(r["resources"],{"maximum_patients","maximum_sites","maximum_levels","maximum_total_iterations","maximum_output_bytes","timeout_seconds"},"resources");
 if len(ids)>integer(z["maximum_patients"],"patients",32,1024)or len(sites)>integer(z["maximum_sites"],"sites",8,64)or len(levels)>integer(z["maximum_levels"],"levels",2,8)or chains*(tune+draws)>integer(z["maximum_total_iterations"],"iterations",1,400000):raise ContractError("resources exceeded")
 output=integer(z["maximum_output_bytes"],"output",1,2*1048576);integer(z["timeout_seconds"],"timeout",1,3600)
 p=obj(r["diagnostic_policy"],{"prior_predictive_draws","maximum_r_hat","minimum_bulk_ess","minimum_tail_ess","minimum_ebfmi","maximum_divergences","maximum_tree_depth_hits","maximum_tree_depth"},"policy")
 return{"ref":ref,"comp":comp,"levels":levels,"sites":sites,"groups":groups,"indicator":np.asarray([x==comp for x in groups]),"outcomes":np.asarray(outcomes,dtype=int),"site_index":np.asarray(site_index,dtype=int),"cut_sd":cut_sd,"site_sd_prior":site_sd,"chains":chains,"tune":tune,"draws":draws,"target":target,"seed":seed,"output":output,"prior":integer(p["prior_predictive_draws"],"prior",1,100000),"rhat":number(p["maximum_r_hat"],"rhat"),"bulk":number(p["minimum_bulk_ess"],"bulk"),"tail":number(p["minimum_tail_ess"],"tail"),"ebfmi":number(p["minimum_ebfmi"],"ebfmi"),"div":integer(p["maximum_divergences"],"div",0,2**63-1),"depth_hits":integer(p["maximum_tree_depth_hits"],"depth_hits",0,2**63-1),"depth":integer(p["maximum_tree_depth"],"depth",1,32)}
def seed_for(seed:int,purpose:str,index:int=0)->int:return int.from_bytes(hashlib.sha256(f"marklab-nonprop-ordinal-v1\0{seed}\0{purpose}\0{index}".encode()).digest()[:4],"little")
def summary(x:np.ndarray)->dict[str,float]:x=x.reshape(-1);return{"mean":float(x.mean()),"sd":float(x.std(ddof=1)),"interval_lower":float(np.quantile(x,.025)),"interval_upper":float(np.quantile(x,.975))}
def sigmoid(x:Any)->Any:return 1/(1+pt.exp(-x))
def np_sigmoid(x:np.ndarray)->np.ndarray:return 1/(1+np.exp(-np.clip(x,-700,700)))
def probs(c:np.ndarray,eta:np.ndarray)->np.ndarray:
 q=np_sigmoid(c-eta[...,None]);return np.diff(np.concatenate([np.zeros((*q.shape[:-1],1)),q,np.ones((*q.shape[:-1],1))],axis=-1),axis=-1)
def tree(t:Any,n:list[str])->np.ndarray:return np.concatenate([np.asarray(t[x].values).reshape(-1)for x in n])
def fit(c:dict[str,Any],request_sha:str,lock:str,worker:str)->dict[str,Any]:
 initial=np.linspace(-1.5,1.5,len(c["levels"])-1)
 with pm.Model():
  ref_cut=pm.Normal("reference_cutpoints",mu=initial,sigma=c["cut_sd"],shape=len(initial),transform=pm.distributions.transforms.ordered,initval=initial)
  comp_cut=pm.Normal("comparison_cutpoints",mu=initial,sigma=c["cut_sd"],shape=len(initial),transform=pm.distributions.transforms.ordered,initval=initial)
  site_sd=pm.HalfNormal("site_intercept_sd",c["site_sd_prior"]);raw=pm.Normal("site_intercept_raw",0,1,shape=len(c["sites"]));site=pm.Deterministic("site_intercept",(raw-raw.mean())*site_sd)
  selected=pt.where(c["indicator"][:,None],comp_cut[None,:],ref_cut[None,:]);cum=sigmoid(selected-site[c["site_index"]][:,None]);prob=pt.concatenate([cum[:,:1],cum[:,1:]-cum[:,:-1],1-cum[:,-1:]],axis=1)
  pm.Categorical("outcome",p=prob,observed=c["outcomes"])
  post=pm.sample(draws=c["draws"],tune=c["tune"],chains=c["chains"],cores=1,blas_cores=1,random_seed=[seed_for(c["seed"],"chain",i)for i in range(c["chains"])],target_accept=c["target"],nuts_sampler="pymc",nuts={"max_treedepth":c["depth"]},progressbar=False,quiet=True,compute_convergence_checks=False)
  pred=pm.sample_posterior_predictive(post,var_names=["outcome"],random_seed=seed_for(c["seed"],"predictive"),progressbar=False)
 d={x:np.asarray(post["posterior"][x].values,dtype=float)for x in["reference_cutpoints","comparison_cutpoints","site_intercept_sd","site_intercept"]};refp=probs(d["reference_cutpoints"][:,:,None,:],d["site_intercept"]);compp=probs(d["comparison_cutpoints"][:,:,None,:],d["site_intercept"]);refm=refp.mean(axis=2);compm=compp.mean(axis=2);rep=np.asarray(pred["posterior_predictive"]["outcome"].values).reshape(-1,len(c["outcomes"]));names=["reference_cutpoints","comparison_cutpoints","site_intercept_sd","site_intercept_raw"]
 prior=bool(np.isfinite(np.random.default_rng(seed_for(c["seed"],"prior")).normal(size=(c["prior"],4))).all());finite=bool(all(np.isfinite(x).all()for x in d.values())and np.isfinite(rep).all());rh=float(tree(pm.stats.rhat(post,var_names=names,method="rank"),names).max());bulk=float(tree(pm.stats.ess(post,var_names=names,method="bulk"),names).min());tail=float(tree(pm.stats.ess(post,var_names=names,method="tail"),names).min());mm=float(tree(pm.stats.mcse(post,var_names=names,method="mean"),names).max());ms=float(tree(pm.stats.mcse(post,var_names=names,method="sd"),names).max());energy=np.asarray(post["sample_stats"]["energy"].values);eb=float(np.min(np.mean(np.diff(energy,axis=1)**2,axis=1)/np.var(energy,axis=1)));div=int(np.asarray(post["sample_stats"]["diverging"].values).sum());depth=int(np.asarray(post["sample_stats"]["reached_max_treedepth"].values).sum());constraints=bool(np.all(np.diff(d["reference_cutpoints"],axis=-1)>0)and np.all(np.diff(d["comparison_cutpoints"],axis=-1)>0)and np.allclose(d["site_intercept"].sum(axis=-1),0,atol=1e-10));complete=prior and finite and constraints and rh<=c["rhat"]and bulk>=c["bulk"]and tail>=c["tail"]and eb>=c["ebfmi"]and div<=c["div"]and depth<=c["depth_hits"]
 rmask=~c["indicator"];cmask=c["indicator"];fr=refm.reshape(-1,len(c["levels"]));fc=compm.reshape(-1,len(c["levels"]));levels=[];ppc=[]
 for i,level in enumerate(c["levels"]):
  a,b=fr[:,i],fc[:,i];or_=float(np.mean(c["outcomes"][rmask]==i));oc=float(np.mean(c["outcomes"][cmask]==i));rr=np.mean(rep[:,rmask]==i,axis=1);rc=np.mean(rep[:,cmask]==i,axis=1);err=abs(or_-a.mean())+abs(oc-b.mean());levels.append({"level":level,"reference_probability":summary(a),"comparison_probability":summary(b),"difference_comparison_minus_reference":summary(b-a)});ppc.append({"level":level,"observed_reference_proportion":or_,"observed_comparison_proportion":oc,"replicated_reference_proportion_mean":float(rr.mean()),"replicated_comparison_proportion_mean":float(rc.mean()),"probability_absolute_replication_error_at_least_observed":float(np.mean(np.abs(rr-a)+np.abs(rc-b)>=err))})
 cut=lambda x:[{"lower_level":c["levels"][i],"upper_level":c["levels"][i+1],**summary(x[...,i])}for i in range(len(c["levels"])-1)]
 return{"format":"marklab.pymc_nonproportional_ordinal_group_site_worker_result","version":1,"backend":{"name":"pymc","version":pm.__version__,"python_version":f"{sys.version_info.major}.{sys.version_info.minor}","environment_lock_sha256":lock,"worker_sha256":worker},"request_sha256":request_sha,"fit_state":"complete"if complete else"nonconverged","sampling":{"chains":c["chains"],"tune_per_chain":c["tune"],"draws_per_chain":c["draws"],"completed_draws":c["chains"]*c["draws"]},"posterior":{"reference_cutpoints":cut(d["reference_cutpoints"]),"comparison_cutpoints":cut(d["comparison_cutpoints"]),"threshold_group_log_odds_effects":[{"threshold_after_level":c["levels"][i],"summary":summary(d["reference_cutpoints"][...,i]-d["comparison_cutpoints"][...,i])}for i in range(len(c["levels"])-1)],"site_intercept_sd":summary(d["site_intercept_sd"]),"sites":[{"site_id":x,"intercept":summary(d["site_intercept"][...,i])}for i,x in enumerate(c["sites"])],"levels":levels,"reference_expected_code":summary((refm*np.arange(len(c["levels"]))).sum(axis=-1)),"comparison_expected_code":summary((compm*np.arange(len(c["levels"]))).sum(axis=-1))},"diagnostics":{"prior_predictive_finite":prior,"posterior_finite":finite,"r_hat":rh,"ess_bulk":bulk,"ess_tail":tail,"mcse_mean":mm,"mcse_sd":ms,"minimum_ebfmi":eb,"divergences":div,"max_tree_depth_hits":depth,"constraints_valid":constraints,"identifiability_checks_passed":True},"posterior_predictive":{"levels":ppc}}
def main()->None:
 if pm.__version__!=PYMC_VERSION or sys.version_info[:2]!=(3,12):raise ContractError("version drift")
 script=Path(__file__);lock=hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest();worker=hashlib.sha256(script.read_bytes()).hexdigest();raw=sys.stdin.buffer.read(2*1048576+1)
 if not raw or len(raw)>2*1048576:raise ContractError("request size invalid")
 c=validate(json.loads(raw),lock,worker);encoded=json.dumps(fit(c,hashlib.sha256(raw).hexdigest(),lock,worker),allow_nan=False,sort_keys=True,separators=(",",":"))
 if len(encoded.encode())>c["output"]:raise ContractError("output too large")
 sys.stdout.write(encoded+"\n")
if __name__=="__main__":
 try:main()
 except Exception as e:print(f"Marklab PyMC nonproportional ordinal worker failed: {type(e).__name__}: {e}",file=sys.stderr);raise SystemExit(2)from e
