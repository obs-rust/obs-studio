# All characterization fragments generate their includes in this binary directory.
if(NOT TARGET OBS::headless-characterization-headers)
  add_library(OBS::headless-characterization-headers INTERFACE IMPORTED)
  set_target_properties(
    OBS::headless-characterization-headers
    PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_BINARY_DIR}"
  )
endif()
