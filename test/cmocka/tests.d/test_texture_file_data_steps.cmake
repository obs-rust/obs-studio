# Recording-session characterization calls the Rust export and real C adapter.
# Both target and test are declared in ../build/test-graph.json.
if(TARGET test_texture_file_data_steps_rust)
  set_property(TARGET test_texture_file_data_steps_rust PROPERTY OBS_RUST_STATIC_ISOLATED TRUE)
endif()
