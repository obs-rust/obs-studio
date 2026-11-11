# The manifest executable is registered by ../build/test-graph.d.
# Keep this auxiliary configure regression with its original toolchain forwarding.
if(ENABLE_CORE_MODULES)
  add_test(
    NAME test_core_module_manifest_source
    COMMAND
      ${CMAKE_COMMAND} "-DOBS_CORE_MODULES_CMAKE=${CMAKE_SOURCE_DIR}/libobs/cmake/obs-core-modules.cmake"
      "-DTEST_FIXTURE_ROOT=${CMAKE_CURRENT_BINARY_DIR}/core_module_manifest_source"
      "-DTEST_GENERATOR=${CMAKE_GENERATOR}" "-DTEST_GENERATOR_PLATFORM=${CMAKE_GENERATOR_PLATFORM}"
      "-DTEST_GENERATOR_TOOLSET=${CMAKE_GENERATOR_TOOLSET}" "-DTEST_GENERATOR_INSTANCE=${CMAKE_GENERATOR_INSTANCE}"
      "-DTEST_BUILD_CONFIG=$<CONFIG>" "-DTEST_C_COMPILER=${CMAKE_C_COMPILER}"
      "-DTEST_MAKE_PROGRAM=${CMAKE_MAKE_PROGRAM}" "-DTEST_TOOLCHAIN_FILE=${CMAKE_TOOLCHAIN_FILE}"
      "-DTEST_OSX_ARCHITECTURES=${CMAKE_OSX_ARCHITECTURES}" -P
      "${CMAKE_CURRENT_SOURCE_DIR}/check_core_module_manifest_source.cmake"
  )
endif()
