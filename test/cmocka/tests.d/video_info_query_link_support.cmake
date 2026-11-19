# Real private adapters in separate lazy members, shared by every Rust consumer.
include("${CMAKE_CURRENT_LIST_DIR}/../rust-static-link-support.cmake")
function(_video_info_query_link_support)
  file(READ "${CMAKE_CURRENT_SOURCE_DIR}/build/rust-static-support.json" manifest)
  string(JSON count LENGTH "${manifest}" sources)
  math(EXPR last "${count} - 1")
  set(sources)
  foreach(index RANGE ${last})
    string(JSON source GET "${manifest}" sources ${index})
    list(APPEND sources "${CMAKE_SOURCE_DIR}/${source}")
  endforeach()
  set_property(
    DIRECTORY
    APPEND
    PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/build/rust-static-support.json"
  )
  add_library(video_info_query_link_support STATIC ${sources})
  target_include_directories(
    video_info_query_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    video_info_query_link_support
    PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
  )
  obs_rust_static_consumers(video_info_query_link_support OBS::libobs-native-dependencies)
endfunction()
cmake_language(DEFER CALL _video_info_query_link_support)
