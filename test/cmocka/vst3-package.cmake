include(${CMAKE_CURRENT_LIST_DIR}/vst3-test-targets.cmake)

set(_vst3_probe "${CMAKE_BINARY_DIR}/vst3-probe")
if(OS_MACOS)
  # These two test binaries are signed ad hoc after staging.
  set_target_properties(test_vst3_host vst3-test-gain PROPERTIES XCODE_ATTRIBUTE_CODE_SIGNING_ALLOWED NO)
  set_target_properties(
    vst3-test-gain
    PROPERTIES
      BUNDLE TRUE
      BUNDLE_EXTENSION vst3
      XCODE_ATTRIBUTE_GENERATE_INFOPLIST_FILE YES
      XCODE_ATTRIBUTE_PRODUCT_BUNDLE_IDENTIFIER com.obsproject.tests.vst3-gain
  )
  set(_vst3_fixture_contents "${_vst3_probe}/OBSFixture.vst3")
  set(
    _vst3_stage_fixture
    "${CMAKE_COMMAND}"
    -E
    copy_directory
    "$<TARGET_BUNDLE_DIR:vst3-test-gain>"
    "${_vst3_fixture_contents}"
  )
  set(
    _vst3_sign_probe
    COMMAND
    /usr/bin/codesign
    --force
    --sign
    -
    "${_vst3_probe}/$<TARGET_FILE_NAME:test_vst3_host>"
    COMMAND
    /usr/bin/codesign
    --force
    --sign
    -
    "${_vst3_fixture_contents}"
  )
else()
  set(_vst3_fixture_contents "${_vst3_probe}/OBSFixture.vst3/Contents/x86_64-win")
  set(
    _vst3_stage_fixture
    "${CMAKE_COMMAND}"
    -E
    copy
    "$<TARGET_FILE:vst3-test-gain>"
    "${_vst3_fixture_contents}/OBSFixture.vst3"
  )
endif()

add_custom_target(
  vst3-package-test
  COMMAND "${CMAKE_COMMAND}" -E make_directory "${_vst3_fixture_contents}"
  COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:test_vst3_host>" "${_vst3_probe}"
  COMMAND ${_vst3_stage_fixture} ${_vst3_sign_probe}
  COMMAND
    "${CMAKE_COMMAND}" -E chdir "${_vst3_probe}" "${CMAKE_COMMAND}" -E tar cf "${_vst3_probe}.tar"
    "$<TARGET_FILE_NAME:test_vst3_host>" OBSFixture.vst3
  DEPENDS test_vst3_host vst3-test-gain
  VERBATIM
)
