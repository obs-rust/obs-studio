# The original Image Slide Show keeps its slides decoded up to a 400 MiB
# memory cap, and must still show every slide past it
# (obs-rust/obs-studio#2279, upstream obsproject/obs-studio#14076). Loads the
# built image-source and obs-transitions modules and generates large PNGs.
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET image-source AND TARGET obs-transitions AND TARGET OBS::libobs-opengl)
  add_executable(test_slideshow_mem_cap test_slideshow_mem_cap.c)
  target_compile_definitions(
    test_slideshow_mem_cap
    PRIVATE
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      IMAGE_SOURCE_MODULE_PATH="$<TARGET_FILE:image-source>"
      IMAGE_SOURCE_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/image-source/data"
      TRANSITIONS_MODULE_PATH="$<TARGET_FILE:obs-transitions>"
      TRANSITIONS_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-transitions/data"
  )
  target_include_directories(test_slideshow_mem_cap PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_slideshow_mem_cap PRIVATE OBS::libobs ZLIB::ZLIB ${CMOCKA_LIBRARIES})
  add_dependencies(test_slideshow_mem_cap image-source obs-transitions libobs-opengl)
  add_dependencies(cmocka-tests test_slideshow_mem_cap)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_slideshow_mem_cap COMMAND test_slideshow_mem_cap WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_slideshow_mem_cap PROPERTIES TIMEOUT 300)
endif()
