# Unchanged characterization of the reachable private render-size bridge.
add_executable(test_video_render_size ../cmocka/test_video_render_size.c)
# The private bridge is not dynamically exported. Compile the real C reference
# implementation into this executable; the Rust variant links the archive.
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/video-render-size-reference.c"
  [=[
#include "libobs/obs-video-render-size-reference.c.h"
]=]
)
target_sources(test_video_render_size PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/video-render-size-reference.c")
target_link_libraries(test_video_render_size PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_video_render_size PRIVATE OBS::w32-pthreads)
endif()
add_test(NAME test_video_render_size COMMAND test_video_render_size)
add_dependencies(cmocka-tests test_video_render_size)

# Reuse the unchanged characterization through the private Rust ABI. Direct
# archive linkage keeps this private symbol out of the public libobs ABI.
if(TARGET libobs_rust-static)
  add_executable(test_video_render_size_rust ../cmocka/test_video_render_size.c)
  target_link_libraries(
    test_video_render_size_rust
    PRIVATE libobs_rust-static OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
  )
  if(OS_WINDOWS)
    target_link_libraries(test_video_render_size_rust PRIVATE OBS::w32-pthreads)
  endif()
  add_test(NAME test_video_render_size_rust COMMAND test_video_render_size_rust)
  add_dependencies(cmocka-tests test_video_render_size_rust)
endif()
