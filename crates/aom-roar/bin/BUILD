# Standalone CLI utilities for Rust Open Audio Renderer (ROAR).

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_test")

rust_binary(
    name = "generate_binaural_filters",
    srcs = [
        "generate_binaural_filters.rs",
        "@aom-roar//src:rice_block_decoder_src",
    ],
    # When building/running this tool with blaze, its directory will be the crate root for relative
    # path of rice_block_decoder.rs.
    crate_root = "generate_binaural_filters.rs",
    edition = "2024",
)

rust_test(
    name = "generate_binaural_filters_test",
    crate = ":generate_binaural_filters",
    edition = "2024",
    deps = [
        "@crates//:googletest",
    ],
)
