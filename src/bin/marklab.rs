#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(all(feature = "allocator-mimalloc", not(feature = "dhat-heap")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "marklab/study.rs"]
mod study;

#[path = "marklab/backend.rs"]
mod backend;
#[path = "marklab/bayes.rs"]
mod bayes;
#[path = "marklab/bayes_advanced.rs"]
mod bayes_advanced;
#[path = "marklab/causal.rs"]
mod causal;
#[path = "marklab/causal_model.rs"]
mod causal_model;
#[path = "marklab/cohort.rs"]
mod cohort;
#[path = "marklab/exclusive_json_output.rs"]
mod exclusive_json_output;
#[path = "marklab/graph.rs"]
mod graph;
#[path = "marklab/local_multivariate.rs"]
mod local_multivariate;
#[path = "marklab/longitudinal.rs"]
mod longitudinal;
#[path = "marklab/multimodal_model.rs"]
mod multimodal_model;
#[path = "marklab/neural.rs"]
mod neural;
#[path = "marklab/numerics.rs"]
mod numerics;
#[path = "marklab/policy.rs"]
mod policy;
#[path = "marklab/project.rs"]
mod project;
#[path = "marklab/registration.rs"]
mod registration;
#[path = "marklab/spatial3d.rs"]
mod spatial3d;
#[path = "marklab/spatial3d_model.rs"]
mod spatial3d_model;
#[path = "marklab/spatial3d_registered.rs"]
mod spatial3d_registered;
#[path = "marklab/topology.rs"]
mod topology;

#[path = "marklab/command_tree.rs"]
mod command_tree;

fn main() -> marklab::Result<()> {
    command_tree::run()
}
