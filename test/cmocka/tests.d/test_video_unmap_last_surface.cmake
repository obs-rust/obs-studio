# C-source characterization only, before the behavior-neutral extraction.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-video.c.h" _unmap_parent)
string(REGEX MATCH "static inline void unmap_last_surface\\([^}]+\\}[^}]+\\}[^}]+\\}" _unmap_body "${_unmap_parent}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/unmap-original.c.h" "${_unmap_body}\n")
add_executable(test_video_unmap_last_surface test_video_unmap_last_surface.c)
target_compile_definitions(
  test_video_unmap_last_surface
  PRIVATE UNMAP_SOURCE="${CMAKE_CURRENT_BINARY_DIR}/unmap-original.c.h"
)
target_include_directories(test_video_unmap_last_surface PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_unmap_last_surface PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash)
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_video_unmap_last_surface PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_video_unmap_last_surface)
add_test(NAME test_video_unmap_last_surface COMMAND test_video_unmap_last_surface)
set_tests_properties(test_video_unmap_last_surface PROPERTIES TIMEOUT 15)
