if(NOT OBS_CORE_MODULES_CMAKE OR NOT TEST_FIXTURE_ROOT)
  message(FATAL_ERROR "OBS_CORE_MODULES_CMAKE and TEST_FIXTURE_ROOT are required")
endif()

set(_evidence "${TEST_FIXTURE_ROOT}/evidence.cmake")
if("$ENV{OBS_TIER2_FIXTURE_PREPARED}" STREQUAL "1")
  if(NOT EXISTS "${_evidence}")
    message(FATAL_ERROR "missing prepared fixture: ${_evidence}")
  endif()
  include("${_evidence}")
  if(NOT _configure_result EQUAL 0 OR NOT _build_result EQUAL 0)
    message(FATAL_ERROR "prepared fixture configure/build failed")
  endif()
  foreach(_path IN LISTS _prepared_files)
    if(NOT EXISTS "${TEST_FIXTURE_ROOT}/${_path}")
      message(FATAL_ERROR "missing prepared fixture: ${_path}")
    endif()
    file(SHA256 "${TEST_FIXTURE_ROOT}/${_path}" _actual)
    if(NOT _actual STREQUAL "${_sha_${_path}}")
      message(FATAL_ERROR "changed prepared fixture: ${_path}")
    endif()
  endforeach()
  # The stale source must remain poisoned; the successful build of the
  # generated binary-tree definition is the original regression assertion.
  file(READ "${TEST_FIXTURE_ROOT}/source/obs-core-modules.c" _stale)
  if(NOT _stale MATCHES "#error stale source-tree")
    message(FATAL_ERROR "changed prepared fixture: stale source is no longer poisoned")
  endif()
  file(SHA256 "${OBS_CORE_MODULES_CMAKE}" _module_sha)
  if(NOT _module_sha STREQUAL _prepared_module_sha)
    message(FATAL_ERROR "changed prepared fixture: core-module build recipe")
  endif()
  return()
endif()

file(REMOVE_RECURSE "${TEST_FIXTURE_ROOT}")
set(_fixture_source_dir "${TEST_FIXTURE_ROOT}/source")
set(_fixture_binary_dir "${TEST_FIXTURE_ROOT}/build")
file(MAKE_DIRECTORY "${_fixture_source_dir}")

file(
  WRITE "${_fixture_source_dir}/CMakeLists.txt"
  [=[
cmake_minimum_required(VERSION 3.28)
project(core_module_manifest_source_fixture C)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/obs-core-modules.c" "int core_module_manifest_fixture(void) { return 1; }\n")
include("${OBS_CORE_MODULES_CMAKE}")
set_target_properties(libobs-core-modules PROPERTIES LINKER_LANGUAGE C)
file(WRITE "${CMAKE_CURRENT_SOURCE_DIR}/main.c" "int core_module_manifest_fixture(void);\nint main(void) { return core_module_manifest_fixture() != 1; }\n")
add_executable(core_module_manifest_fixture main.c)
target_link_libraries(core_module_manifest_fixture PRIVATE OBS::libobs-core-modules)
]=]
)
file(WRITE "${_fixture_source_dir}/obs-core-modules.h" "/* fixture header */\n")
file(WRITE "${_fixture_source_dir}/obs-core-modules.c" "#error stale source-tree obs-core-modules.c was selected\n")

set(
  _configure_command
  "${CMAKE_COMMAND}"
  -S
  "${_fixture_source_dir}"
  -B
  "${_fixture_binary_dir}"
)
if(TEST_GENERATOR)
  list(APPEND _configure_command -G "${TEST_GENERATOR}")
endif()
if(TEST_GENERATOR_PLATFORM)
  list(APPEND _configure_command -A "${TEST_GENERATOR_PLATFORM}")
endif()
if(TEST_GENERATOR_TOOLSET)
  list(APPEND _configure_command -T "${TEST_GENERATOR_TOOLSET}")
endif()
if(TEST_GENERATOR_INSTANCE)
  list(APPEND _configure_command "-DCMAKE_GENERATOR_INSTANCE=${TEST_GENERATOR_INSTANCE}")
endif()
list(APPEND _configure_command "-DOBS_CORE_MODULES_CMAKE=${OBS_CORE_MODULES_CMAKE}")
if(TEST_C_COMPILER)
  list(APPEND _configure_command "-DCMAKE_C_COMPILER=${TEST_C_COMPILER}")
endif()
if(TEST_MAKE_PROGRAM)
  list(APPEND _configure_command "-DCMAKE_MAKE_PROGRAM=${TEST_MAKE_PROGRAM}")
endif()
if(TEST_TOOLCHAIN_FILE)
  list(APPEND _configure_command "-DCMAKE_TOOLCHAIN_FILE=${TEST_TOOLCHAIN_FILE}")
endif()
if(TEST_OSX_ARCHITECTURES)
  list(APPEND _configure_command "-DCMAKE_OSX_ARCHITECTURES=${TEST_OSX_ARCHITECTURES}")
endif()

execute_process(
  COMMAND ${_configure_command}
  RESULT_VARIABLE _configure_result
  OUTPUT_VARIABLE _configure_stdout
  ERROR_VARIABLE _configure_stderr
)
if(NOT _configure_result EQUAL 0)
  file(REMOVE_RECURSE "${TEST_FIXTURE_ROOT}")
  message(FATAL_ERROR "Unable to configure stale-manifest fixture:\n${_configure_stdout}${_configure_stderr}")
endif()

set(_build_command "${CMAKE_COMMAND}" --build "${_fixture_binary_dir}")
if(TEST_BUILD_CONFIG)
  list(APPEND _build_command --config "${TEST_BUILD_CONFIG}")
endif()
list(APPEND _build_command --target core_module_manifest_fixture --parallel 6)
execute_process(
  COMMAND ${_build_command}
  RESULT_VARIABLE _build_result
  OUTPUT_VARIABLE _build_stdout
  ERROR_VARIABLE _build_stderr
)
if(NOT _build_result EQUAL 0)
  message(
    FATAL_ERROR
    "The core-module target did not compile the generated binary-tree manifest when a stale source-tree copy exists:\n${_build_stdout}${_build_stderr}"
  )
endif()
if(TEST_PREPARE_ONLY)
  file(SHA256 "${OBS_CORE_MODULES_CMAKE}" _module_sha)
  file(WRITE "${_evidence}" "set(_configure_result 0)\nset(_build_result 0)\nset(_prepared_module_sha ${_module_sha})\n")
  file(GLOB_RECURSE _prepared_files RELATIVE "${TEST_FIXTURE_ROOT}" "${TEST_FIXTURE_ROOT}/*")
  list(REMOVE_ITEM _prepared_files evidence.cmake)
  # Hash source, generated source, build graph and products. Relative names
  # allow a common build-root binding without rewriting the evidence.
  foreach(_path IN LISTS _prepared_files)
    file(SHA256 "${TEST_FIXTURE_ROOT}/${_path}" _sha)
    file(APPEND "${_evidence}" "list(APPEND _prepared_files [==[${_path}]==])\nset([==[_sha_${_path}]==] ${_sha})\n")
  endforeach()
else()
  file(REMOVE_RECURSE "${TEST_FIXTURE_ROOT}")
endif()
