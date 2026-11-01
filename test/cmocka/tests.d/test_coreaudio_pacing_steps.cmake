# Characterization of the CoreAudio monitor's queue pacing of
# libobs/audio-monitoring/osx/coreaudio-output.c, moved to
# libobs/audio-monitoring/osx/coreaudio-pacing.c and ported to
# rust/obs-audio-monitoring (coreaudio_pacing): the buffer and wait sizes,
# the fill test, the volume gate, the end of the wait, the pause once every
# buffer is back and what follows a failed enqueue. A stand-in monitor puts
# them together in the C's order. Builds the moved file on its own: no Apple
# header and no libobs core, so it runs on every platform. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. The Tier 3 parity tests in rust/obs-audio-monitoring compare the shims
# with this C too.
# Target and test are declared in ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target), with
# COREAUDIO_PACING_TEST_RUST: the port's intentional difference (a failed
# enqueue is recovered) in place of the C's three failed-enqueue cases.
# coreaudio_monitor_* are hidden in the shared library, so the test takes them
# from the production staticlib at executable link time and compiles none of
# the C. It includes only coreaudio-pacing.h, and the obs-audio-monitoring
# member needs nothing from libobs, so it links the archive alone. Linux only:
# the shims have no host code, so one host shows the swap.
if(TARGET libobs_rust-static)
  if(OS_LINUX)
    add_executable(test_coreaudio_pacing_steps_rust test_coreaudio_pacing_steps.c)
    target_include_directories(
      test_coreaudio_pacing_steps_rust
      PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
    )
    target_compile_definitions(test_coreaudio_pacing_steps_rust PRIVATE COREAUDIO_PACING_TEST_RUST)
    target_link_libraries(test_coreaudio_pacing_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES} m)
    obs_test_sanitize(test_coreaudio_pacing_steps_rust "address,undefined")
    add_dependencies(cmocka-tests test_coreaudio_pacing_steps_rust)
    add_test(test_coreaudio_pacing_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_coreaudio_pacing_steps_rust)
  endif()
endif()
