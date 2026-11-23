if(NOT UNIX)
  return()
endif()

# Inject final-close errors through libc, without changing libobs.
add_library(config_close_fault SHARED "${CMAKE_CURRENT_LIST_DIR}/../config_close_fault.c")
set_target_properties(
  config_close_fault
  PROPERTIES C_VISIBILITY_PRESET default XCODE_ATTRIBUTE_GCC_SYMBOLS_PRIVATE_EXTERN NO
)
target_link_libraries(config_close_fault PRIVATE ${CMAKE_DL_LIBS})
# Keep the preload helper a shared library, independent of the test executable.
set_target_properties(
  config_close_fault
  PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/config_close_fault$<0:>"
    OUTPUT_NAME config_close_fault
    PREFIX ""
    SUFFIX ".preload"
)
# Executable/CTest registration is shared with Cargo in ../build/test-graph.d.
