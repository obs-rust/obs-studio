# Assertion half of the original compile-only stale-manifest regression.
# The immutable Linux artifact contains the receipt of running the original,
# unchanged CMake fixture with the Windows release toolchain.
if(NOT RECEIPT OR NOT EXPECTED_SOURCE_SHA OR NOT SOURCE_ROOT)
  message(FATAL_ERROR "Require receipt, source root and immutable source SHA")
endif()
file(READ "${RECEIPT}" _receipt)
foreach(
  _pair
  IN
  ITEMS
    "schema;1"
    "test;test_core_module_manifest_source"
    "target;x86_64-pc-windows-msvc"
    "profile;RelWithDebInfo"
    "exit_code;0"
    "source_sha;${EXPECTED_SOURCE_SHA}"
)
  list(GET _pair 0 _field)
  list(GET _pair 1 _expected)
  string(JSON _actual GET "${_receipt}" "${_field}")
  if(NOT _actual STREQUAL _expected)
    message(FATAL_ERROR "Invalid Linux fixture receipt field ${_field}")
  endif()
endforeach()
foreach(_input IN ITEMS test/cmocka/check_core_module_manifest_source.cmake libobs/cmake/obs-core-modules.cmake)
  string(JSON _expected GET "${_receipt}" inputs "${_input}")
  file(SHA256 "${SOURCE_ROOT}/${_input}" _actual)
  if(NOT _actual STREQUAL _expected)
    message(FATAL_ERROR "Fixture input changed after Linux preparation: ${_input}")
  endif()
endforeach()
