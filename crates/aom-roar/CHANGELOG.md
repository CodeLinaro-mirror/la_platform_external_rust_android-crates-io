# Change Log

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Valid sections are:

-   'Added' for new features.
-   'Changed' for changes in existing functionality.
-   'Deprecated' for soon-to-be removed features.
-   'Removed' for now removed features.
-   'Fixed' for any bug fixes.
-   'Security' in case of vulnerabilities.

## [Unreleased]

Changes in the repo not yet in a release will appear here.

## [1.1.0] - 2026-10-29

New release equivalent to OAR at
[OAR commit a7a5cc8](https://github.com/AOMediaCodec/oar/commit/a7a5cc8).

### [Added]

-   Added standalone public C/C++ header `include/roar.h` and Bazel `cc_library`
    target `include:roar` so C/C++ consumers do not need manual forward
    declarations or `liboar` headers. Bazel Rust users could still use
    `src:aom_roar_rlib`.

### [Changed]

-   Prefixed C API types, enum constants, and macros in `include/roar.h` (and
    `aom_roar::c_types`) with `roar_` / `ck_roar_` / `ROAR_` to match the
    `roar_*` C API functions and prevent symbol collisions with `liboar`.
-   Separated `Layout` type into `InputLayout` and `OutputLayout` for the Rust
    API.
-   Changed the way the different types of filter coefficients and matrices are
    stored in the binary in order to shrink the binary size. Effected data
    include SH HRIR filters for binaural rendering, matrices for channel-to-
    channel rendering, and matrices for ambisonic-to-channel rendering. Runtime
    performance is unaffected or improved and rendering remains equivalent to
    OAR (< 1e-6).

### [Fixed]

-   Fixed building and running tests with Bazelisk and Cargo.
-   Fixed rendering when input and output layouts are both binaural. Previously,
    OBR just returned an error.
-   Fixed improper routing of channels when rendering ambisonics to Sound System
    H. (Reported in
    [roar/issues/1](https://github.com/AOMediaCodec/roar/issues/1).)
-   Fixed binaural rendering when world-locked and head-locked content are in
    the same audio group. (Reported in
    [oar/issues/53](https://github.com/AOMediaCodec/oar/issues/53).)
-   Fixed handling of downmix modes to align with OAR
    ([oar/issues/50](https://github.com/AOMediaCodec/oar/issues/50)). Downmix
    modes are now correctly applied, ignored, or rejected based on the element's
    active rendering path and whether or not it is a valid downmix mode for that
    element.

## [1.0.0] - 2026-10-15

### [Added]

-   [Matching the new C-API functions in OAR](https://github.com/AOMediaCodec/oar/pull/40),
    there are new API methods for controlling the limiter:
    `roar_set_limiter_threshold`, `roar_get_limiter_delay`, and `roar_flush`.

### [Changed]

-   Input audio is now discarded after a call to `roar_render`.
-   Matching [a change in OAR](https://github.com/AOMediaCodec/oar/pull/35),
    animated (linear, bezier) gains are now interpolated in the decibel domain.
-   Matching
    [the new limiter in OAR](https://github.com/AOMediaCodec/oar/pull/40), the
    simple limiter is replaced with a look-ahead limiter.
-   Using `roar_set_metadata_unit_to_process` was accepted for binaural output
    but was not actually applied. It is now explicitly allowed (in the README),
    works correctly, and is tested.
-   When output format is **binaural** and audio element **object positions**
    are updated with `roar_update_audio_element_metadata` (C API) or
    `RoarRenderer::update_element_positions` (Rust API) before the first render,
    the object is now instantly updated to the new position rather than ramping
    to the position over the first render. This behavior was changed in OAR
    ([oar/issues/21](https://github.com/AOMediaCodec/oar/issues/21),
    [oar/pull/24](https://github.com/AOMediaCodec/oar/pull/24)).

### [Fixed]

-   [Matching the fixes in OAR](https://github.com/AOMediaCodec/oar/pull/14),
    when binaural output is requested and the output sample rate is not 48kHz,
    the output audio is now appropriately scaled and the interpolation filter
    tail is no longer truncated.

## [0.1.0] - 2026-09-23

### [Added]

-   Initial release, supporting a C API compatible with OAR's C API.
