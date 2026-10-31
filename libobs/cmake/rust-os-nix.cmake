target_link_options(${OBS_LIBOBS_RUST_TARGET} PRIVATE "LINKER:--version-script=${_obs_rust_export_map}")
set_property(TARGET ${OBS_LIBOBS_RUST_TARGET} APPEND PROPERTY LINK_DEPENDS "${_obs_rust_export_map}")
# The obs-data glue calls json_delete from inside the staticlib.
# jansson is first linked earlier, in libobs/CMakeLists.txt. On
# Ubuntu, GNU ld --as-needed drops that first occurrence because
# nothing references jansson yet, and every cmocka executable then
# fails to link. Link it again so a later occurrence follows the
# archive.
if(OS_LINUX)
  target_link_libraries(${OBS_LIBOBS_RUST_TARGET} PRIVATE jansson::jansson)
endif()
