# Public C/C++ header target for Rust Open Audio Renderer (ROAR).

load("@rules_cc//cc:cc_library.bzl", "cc_library")

cc_library(
    name = "roar",
    hdrs = ["roar.h"],
    visibility = ["//visibility:public"],
    deps = [
        "@aom-roar//src:aom_roar",  # buildcleaner: keep
    ],
)
