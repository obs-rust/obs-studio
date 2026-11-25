# Recording-session characterization calls the Rust export and real C adapter.
# Both target and test are declared in ../build/test-graph.json.
function(_obs_isolate_texture_file_data_recording)
  if(TARGET test_texture_file_data_steps_rust)
    set_property(TARGET test_texture_file_data_steps_rust PROPERTY OBS_RUST_STATIC_ISOLATED TRUE)
  endif()
endfunction()

# Fragments are read before canonical graph targets are created.
cmake_language(DEFER CALL _obs_isolate_texture_file_data_recording)
