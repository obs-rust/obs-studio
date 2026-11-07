if(OS_MACOS)
  add_executable(test_vt_encoder_list_once test_vt_encoder_list_once.c)
  target_link_libraries(
    test_vt_encoder_list_once
    PRIVATE
      OBS::libobs
      "-framework CoreFoundation"
      "-framework CoreMedia"
      "-framework CoreVideo"
      "-framework VideoToolbox"
  )
  target_compile_options(test_vt_encoder_list_once PRIVATE -fblocks)
  target_compile_definitions(test_vt_encoder_list_once PRIVATE $<$<BOOL:${ENABLE_HEVC}>:ENABLE_HEVC>)
  add_dependencies(cmocka-tests test_vt_encoder_list_once)
  add_test(NAME test_vt_encoder_list_once COMMAND test_vt_encoder_list_once)
endif()
