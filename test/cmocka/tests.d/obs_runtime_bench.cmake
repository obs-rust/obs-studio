# A headless runtime workload for libobs (#3385): renders and mixes a fixed
# scene on libobs-opengl and prints JSON for ci/benchmark_libobs.py
# --runtime. The ctest case is a 1 s smoke run (77 = skipped without a
# display); xvfb-run provides one on Linux CI.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
