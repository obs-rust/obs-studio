# May also be supplied as CMAKE_PROJECT_INCLUDE to the unmodified native graph.
# Capture provenance only after the canonical owner/test graph is configured.
include_guard(GLOBAL)
function(_obs_tier2_registry_evidence)
  execute_process(
    COMMAND
      uv run --no-project "${CMAKE_SOURCE_DIR}/ci/macos_tier2_registry.py" record --source "${CMAKE_SOURCE_DIR}" --build
      "${CMAKE_BINARY_DIR}"
    COMMAND_ERROR_IS_FATAL ANY
  )
endfunction()
cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL _obs_tier2_registry_evidence)
