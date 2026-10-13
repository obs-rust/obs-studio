# The private PipeWire daemon the PipeWire tests run against must start with
# a long TMPDIR, and a daemon that fails must fail the test, not skip it.
# Needs pipewire installed; skips (77) without it, as in CI.
if(OS_LINUX AND TARGET OBS::pipewire)
  add_test(
    NAME test_pipewire_wrapper
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_wrapper.sh" "${CMAKE_CURRENT_SOURCE_DIR}"
  )
  set_tests_properties(test_pipewire_wrapper PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 300)
endif()
