#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(all(feature = "allocator-mimalloc", not(feature = "dhat-heap")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> marklab::Result<()> {
    marklab::run_cli()
}
