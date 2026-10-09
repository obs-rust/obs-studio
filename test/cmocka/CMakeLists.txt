project(obs-cmocka)

find_package(CMocka CONFIG REQUIRED)

# Every test executable joins this target, so the Tier 2 validators
# (rust/tools/*-validate) build `cmocka-tests` instead of a hand-kept list and a
# new test cannot go unbuilt (and show up in ctest as "Not Run").
add_custom_target(cmocka-tests)

# Serializer test
add_executable(test_serializer test_serializer.c)
target_include_directories(test_serializer PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_serializer PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_serializer)
add_test(test_serializer ${CMAKE_CURRENT_BINARY_DIR}/test_serializer)

# darray test
add_executable(test_darray test_darray.c)
target_include_directories(test_darray PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_darray PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_darray)
add_test(test_darray ${CMAKE_CURRENT_BINARY_DIR}/test_darray)

# bitstream test
add_executable(test_bitstream test_bitstream.c)
target_include_directories(test_bitstream PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_bitstream PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_bitstream)
add_test(test_bitstream ${CMAKE_CURRENT_BINARY_DIR}/test_bitstream)

# OS path test
add_executable(test_os_path test_os_path.c)
target_include_directories(test_os_path PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_os_path PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_os_path)
add_test(test_os_path ${CMAKE_CURRENT_BINARY_DIR}/test_os_path)

# /dev/full reproduces delayed disk-write errors without fault injection.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  add_executable(test_quick_write test_quick_write.c)
  target_include_directories(test_quick_write PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_quick_write PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_quick_write)
  add_test(test_quick_write ${CMAKE_CURRENT_BINARY_DIR}/test_quick_write)
endif()

# AVC header test. Where the toolchain supports it, obs-avc.c is compiled into
# the test with UBSan (no recovery) so undefined behavior fails the test
# (obs-rust/obs-studio#11); its definitions take precedence over libobs's.
add_executable(test_avc test_avc.c)
target_include_directories(test_avc PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_avc PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_sources(test_avc PRIVATE "${CMAKE_SOURCE_DIR}/libobs/obs-avc.c")
  target_compile_options(test_avc PRIVATE -fsanitize=undefined -fno-sanitize-recover=undefined)
  target_link_options(test_avc PRIVATE -fsanitize=undefined)
endif()

add_dependencies(cmocka-tests test_avc)
add_test(test_avc ${CMAKE_CURRENT_BINARY_DIR}/test_avc)

# Formatted filename test (obs-rust/obs-studio#21). platform.c is compiled into
# the test so the test's obs_get_video_info fixture replaces libobs's and no obs
# core has to be started; its definitions take precedence over libobs's. utf8.c
# is not exported by libobs, and platform.c needs it.
if(NOT MSVC)
  add_executable(
    test_formatted_filename
    test_formatted_filename.c
    "${CMAKE_SOURCE_DIR}/libobs/util/platform.c"
    "${CMAKE_SOURCE_DIR}/libobs/util/utf8.c"
  )
  target_include_directories(test_formatted_filename PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_formatted_filename PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  target_compile_options(test_formatted_filename PRIVATE -fsanitize=address,undefined -fno-sanitize-recover=undefined)
  target_link_options(test_formatted_filename PRIVATE -fsanitize=address,undefined)

  add_dependencies(cmocka-tests test_formatted_filename)
  add_test(test_formatted_filename ${CMAKE_CURRENT_BINARY_DIR}/test_formatted_filename)
endif()
