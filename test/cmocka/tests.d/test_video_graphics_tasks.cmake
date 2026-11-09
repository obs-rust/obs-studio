# Public graphics queue characterization needs a real OpenGL graphics thread.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_video_graphics_tasks test_video_graphics_tasks.c)
  target_link_libraries(test_video_graphics_tasks PRIVATE OBS::libobs OBS::native-test-cmocka Threads::Threads)
  target_compile_definitions(test_video_graphics_tasks PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  add_dependencies(test_video_graphics_tasks OBS::libobs-opengl)
  add_dependencies(cmocka-tests test_video_graphics_tasks)
  find_program(XVFB_RUN xvfb-run)
  if(XVFB_RUN)
    add_test(NAME test_video_graphics_tasks COMMAND ${XVFB_RUN} -a $<TARGET_FILE:test_video_graphics_tasks>)
  else()
    add_test(NAME test_video_graphics_tasks COMMAND test_video_graphics_tasks)
  endif()
  set_tests_properties(
    test_video_graphics_tasks
    PROPERTIES WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/libobs" TIMEOUT 60 LABELS graphics
  )
endif()
