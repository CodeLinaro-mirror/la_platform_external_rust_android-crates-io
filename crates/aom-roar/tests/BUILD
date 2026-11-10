# Tests for Rust Open Audio Renderer (ROAR).

load("@rules_rust//rust:defs.bzl", "rust_test")

rust_test(
    name = "allocation_tests",
    srcs = [
        "allocation_tests.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "combination_rendering_test",
    srcs = [
        "combination_rendering_test.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "channel_based_rendering_test",
    srcs = [
        "channel_based_rendering_test.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "limiter_test",
    srcs = [
        "limiter_test.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "object_based_rendering_test",
    srcs = [
        "object_based_rendering_test.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "panic_boundary_tests",
    srcs = [
        "panic_boundary_tests.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "scene_based_rendering_test",
    srcs = [
        "scene_based_rendering_test.rs",
        "test_helpers/mod.rs",
    ],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)

rust_test(
    name = "validation_test",
    srcs = ["validation_test.rs"],
    edition = "2024",
    deps = [
        "@aom-roar//src:aom_roar_rlib",
        "@crates//:googletest",
    ],
)
