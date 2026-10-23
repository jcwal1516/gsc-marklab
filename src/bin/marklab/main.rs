#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(all(feature = "allocator-mimalloc", not(feature = "dhat-heap")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod backend;
mod bayes;
mod bayes_advanced;
mod bayes_thomas_cluster;
mod causal;
mod causal_model;
mod cli_error;
mod co_occurrence;
mod codecs;
mod cohort;
mod command_tree;
mod exclusive_json_output;
mod graph;
mod input_file;
mod local_multivariate;
mod longitudinal;
mod multimodal_model;
mod neural;
mod numerics;
mod pathology;
mod policy;
mod project;
mod projected_variogram_kernel;
mod registration;
mod spatial3d;
mod spatial3d_model;
mod spatial3d_registered;
mod study;
mod topology;
mod worker_process;

fn main() -> marklab::Result<()> {
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new(
            "--internal-projected-variogram-kernel",
        ))
    {
        return projected_variogram_kernel::run().map_err(Into::into);
    }
    command_tree::run()
}
