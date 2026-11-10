# Performance benchmarking comparing ROAR (Rust) and liboar (C/C++).

load("@rules_cc//cc:cc_library.bzl", "cc_library")
load("@rules_cc//cc:cc_test.bzl", "cc_test")

cc_library(
    name = "benchmark_helpers",
    testonly = True,
    srcs = ["benchmark_helpers.cc"],
    hdrs = ["benchmark_helpers.h"],
    deps = [
        "@abseil-cpp//absl/log:absl_check",
        "@abseil-cpp//absl/types:span",
        "@aom-roar//equivalence_testing:test_helpers",
        "@aom-roar//include:roar",
    ],
)

cc_test(
    name = "benchmarks",
    srcs = ["benchmarks.cc"],
    deps = [
        ":benchmark_helpers",
        "@abseil-cpp//absl/base:log_severity",
        "@abseil-cpp//absl/log:absl_check",
        "@abseil-cpp//absl/log:globals",
        "@abseil-cpp//absl/types:span",
        "@aom-roar//equivalence_testing:test_helpers",
        "@aom-roar//include:roar",
        "@google_benchmark//:benchmark_main",
    ],
)
