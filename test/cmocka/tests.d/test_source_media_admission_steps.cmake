# Characterization of the media action admission decisions of
# libobs/obs-source.c (the obs_source_media_*() functions and
# process_media_actions()), moved to libobs/obs-source-media-admission.c and
# ported to rust/obs-source (media_admission): every call with every output
# flag set, valid or not and with or without its callback, in the callers'
# order with the values each step reads, the getters' fallbacks, the dispatch
# and signal of every queued action type in the enum and out, and a queue run
# through in order. Builds the moved file into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. test_media_play_after_stop, test_media_time, test_slideshow_manual_end
# and test_stinger_media_duration cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the eight hidden functions, and includes their header and obs.h
# (for the libobs values the header mirrors; obs.h pulls SIMDe and
# obsconfig.h, hence libobs's include directories), so it links the archive
# alone: its obs-source member needs nothing from libobs or the other crates.
# Linking the archive directly avoids exporting private symbols just for a
# test.
if(TARGET libobs_rust-static)
  add_executable(test_source_media_admission_steps_rust test_source_media_admission_steps.c)
  target_include_directories(
    test_source_media_admission_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_media_admission_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_source_media_admission_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_source_media_admission_steps_rust)
  add_test(test_source_media_admission_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_media_admission_steps_rust)
endif()
