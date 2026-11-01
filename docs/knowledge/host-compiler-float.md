---
type: concept
title: Float results depend on the host compiler
description: The CI runner's gcc contracts mul+add into an FMA and the Tier 2 image's gcc does not, so the same C source gives different float results.
sources: ["#3422", "#3362"]
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
- Rust does not contract `a * b + c` on its own; match the C reference built
  without contraction (the sanitizer `valgrind` build already passes
  `-ffp-contract=off`), or compare with a tolerance where the C contract allows it.
- Follow #3422 for the build-wide decision (pin `-ffp-contract`); this concept
  is stale once that lands or the runner or Tier 2 image changes its gcc.

## Why `stale_after`

The finding depends on the runner image and the Tier 2 image. Re-check it after
the date or any compiler/image bump, and update or retire this file.
