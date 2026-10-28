#![no_main]

use libfuzzer_sys::fuzz_target;
use marklab_microbio::parse_bruker_flex;

// The first byte sets where the remaining bytes split into the acqu text and the fid payload.
fuzz_target!(|bytes: &[u8]| {
    if let Some((&split, rest)) = bytes.split_first() {
        let (acqu, fid) = rest.split_at(usize::from(split) * rest.len() / 255);
        let _ = parse_bruker_flex(acqu, fid);
    }
});
