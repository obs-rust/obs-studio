# ep_free is hidden: characterize the extracted original C source, not a
# future Rust replacement. libobs supplies the allocator and lexer helpers.
add_executable(
  test_effect_parser_lifecycle
  test_effect_parser_lifecycle.c
  "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle.c.h"
)
target_include_directories(test_effect_parser_lifecycle PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_effect_parser_lifecycle PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_effect_parser_lifecycle "address,undefined")
add_dependencies(cmocka-tests test_effect_parser_lifecycle)
add_test(test_effect_parser_lifecycle ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_lifecycle)

# The Rust staticlib exists only with the hybrid backend (OBS_LIBOBS_BACKEND=C
# has no Rust target), so this variant is built and run only there.
if(TARGET libobs_rust-static)
  # Same immutable C assertions against the exact production Rust staticlib.
  # ep_free is hidden in the shared library, so select it at executable link
  # time. The C inline-destructor adapter is compiled here as it is in libobs.
  add_executable(
    test_effect_parser_lifecycle_rust
    test_effect_parser_lifecycle.c
    "${CMAKE_SOURCE_DIR}/libobs/graphics/effect-parser-lifecycle-adapter.c.h"
  )
  target_include_directories(test_effect_parser_lifecycle_rust PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_effect_parser_lifecycle_rust
    PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES} libobs_rust-static
  )
  obs_test_sanitize(test_effect_parser_lifecycle_rust "address,undefined")
  add_dependencies(cmocka-tests test_effect_parser_lifecycle_rust)
  add_test(test_effect_parser_lifecycle_rust ${CMAKE_CURRENT_BINARY_DIR}/test_effect_parser_lifecycle_rust)
endif()
