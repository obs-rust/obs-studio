# Real private adapters in separate lazy members, shared by every Rust consumer.
include("${CMAKE_CURRENT_LIST_DIR}/../rust-static-link-support.cmake")
function(_video_info_query_link_support)
  obs_add_rust_static_support("${CMAKE_SOURCE_DIR}" OBS::libobs-native-dependencies)
endfunction()
cmake_language(DEFER CALL _video_info_query_link_support)
