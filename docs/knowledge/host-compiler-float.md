---
type: concept
title: Float results depend on the host compiler
description: The CI runner's gcc contracts mul+add into an FMA and the Tier 2 image's gcc does not, so the same C source gives different float results.
sources: ["#3422", "#3362", "#3428", "#3489"]
status: active
stale_after: 2027-01-01
---

# Float results depend on the host compiler

## The trap

On the GitHub `ubuntu-26.04` runners, gcc fuses a multiply and an add into one
fused multiply-add (FMA) by default. In the Tier 2 Docker image, gcc 15.2.0
(the same version) defaults to `-march=x86-64` without FMA and emits a separate
multiply and add. Same source, same command line (only `-O2`, no `-march`,
`-mfma` or `-ffp-contract`), different machine code, different last bit.

## How it showed up

#3362 / #3422: `obs_source_transition_render_offset()`
(`libobs/obs-source-transition-render.c.h`) compiled to `vfnmadd231ss` in the
sanitizer job and to `mulss` + `subss` in Tier 2. `1920 - 1000 * 1.08f` is
`840.0` with the product rounded to `float` first, and differs in the last
bit when fused. A differential test that compares C and Rust exactly passed in
one place and failed in the other.

## What to do instead

- When a C-vs-Rust or golden comparison of floats differs only in the last bit
  between hosts, suspect FMA contraction first; check the disassembly
  (`objdump -d`) of the C function on both hosts before changing Rust.
- Rust does not contract `a * b + c` on its own, so every C build that is
  compared with Rust or with golden floats compiles without contraction
  (#3422, option B): the CMake test build (`-ffp-contract=off` on every C
  and C++ target when `ENABLE_UNIT_TESTS` is on, in the Linux and macOS
  `compilerconfig.cmake`), the test graph that `obs-build`
  compiles (`configure_c_tests`), the C oracle (`rust/obs-c-oracle/build.rs`)
  and the three sanitizer passes (`TOOL_FLAGS` in `sanitize.py`, #3428). MSVC
  does not contract under its default `/fp:precise`.
- App and release builds keep the compiler default, so the shipped C may round
  the last bit differently from the Rust port on a host that contracts. Don't
  pin a shipped-build float result in a test.
- A new C test, oracle or parity build compiles with `-ffp-contract=off` too.
  `test_float_contraction` has no flag of its own and fails when the CMake
  test build loses it on a host that contracts (the hosted runners, macOS
  arm64). Reproduce it locally with `-mfma`: `gcc -O2 -mfma` gives
  `839.99994`.

## Why `stale_after`

The finding depends on the runner image and the Tier 2 image. Re-check it after
the date or any compiler/image bump, and update or retire this file.
