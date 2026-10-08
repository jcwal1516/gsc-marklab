pub fn expected_runtime_features() -> serde_json::Value {
    let features = [
        (cfg!(feature = "allocator-mimalloc"), "allocator-mimalloc"),
        (cfg!(feature = "cli"), "cli"),
        (cfg!(feature = "csv"), "csv"),
        (cfg!(feature = "dhat-heap"), "dhat-heap"),
        (cfg!(feature = "parallel"), "parallel"),
        (cfg!(feature = "parquet"), "parquet"),
        (cfg!(feature = "wsi"), "wsi"),
    ]
    .into_iter()
    .filter_map(|(enabled, name)| enabled.then_some(name))
    .collect::<Vec<_>>();

    serde_json::json!(features)
}
