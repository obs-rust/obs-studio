# Characterization of the scene item crop arithmetic in
# libobs/obs-scene-crop.c.h, ported to rust/obs-scene (crop): the four
# functions on their own, then the setter's clamp, crop sums and a nested
# scene's rescale through the real libobs. The functions are hidden in
# libobs, so the test compiles the C file itself. The real-libobs cases need
# core video, so it runs (under xvfb-run) on Linux only.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls the four hidden functions and the exported scene API. libobs comes
# first, so it gives the exported functions, and the archive gives only the
# four hidden ones (with the archive first, its obs-data members are pulled
# in and need libjansson). Linking the archive directly avoids exporting
# private symbols just for a test.
if(TARGET libobs_rust-static)
  if(OS_LINUX AND TARGET OBS::libobs-opengl)
    add_executable(test_scene_item_crop_rust test_scene_item_crop.c)
    target_compile_definitions(
      test_scene_item_crop_rust
      PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
    )
    target_include_directories(test_scene_item_crop_rust PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_scene_item_crop_rust PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES} m)
    add_dependencies(test_scene_item_crop_rust libobs-opengl)
    add_dependencies(cmocka-tests test_scene_item_crop_rust)
    # libobs finds its effect files relative to the working directory.
    add_test(
      NAME test_scene_item_crop_rust
      COMMAND test_scene_item_crop_rust
      WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
    )
    set_tests_properties(test_scene_item_crop_rust PROPERTIES TIMEOUT 120)
  endif()
endif()
