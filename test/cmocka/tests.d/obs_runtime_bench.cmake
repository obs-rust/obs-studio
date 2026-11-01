# A headless runtime workload for libobs (#3385): renders and mixes a fixed
# scene on libobs-opengl and prints JSON for ci/benchmark_libobs.py
# --runtime. The ctest case is a 1 s smoke run (77 = skipped without a
# display); xvfb-run provides one on Linux CI.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(obs_runtime_bench obs_runtime_bench.c)
  target_compile_definitions(obs_runtime_bench PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_link_libraries(obs_runtime_bench PRIVATE OBS::libobs m)
  add_dependencies(obs_runtime_bench libobs-opengl)
  add_dependencies(cmocka-tests obs_runtime_bench)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME obs_runtime_bench_smoke
    COMMAND obs_runtime_bench --seconds 1 --warmup 0.5
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(obs_runtime_bench_smoke PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
endif()
