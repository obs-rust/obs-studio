# Target-local headers shared with obs-build's explicit @NAME@ substitutions.
# Values are data, never evaluated as CMake code or runtime path templates.
function(_obs_test_graph_configured_header json name root output)
  _obs_test_graph_fields("${json}" "targets;${name};configured_headers;${output}" "input;values")
  string(
    JSON input
    GET "${json}"
    targets
    "${name}"
    configured_headers
    "${output}"
    input
  )
  if(
    input STREQUAL ""
    OR IS_ABSOLUTE "${input}"
    OR input MATCHES "[\\\\:;\n\r\t]|(^|/)\\.\\.?(/|$)|//|/$"
    OR NOT output MATCHES "^[A-Za-z0-9_-][A-Za-z0-9_.-]*\\.h$"
  )
    message(FATAL_ERROR "Invalid native test configured header ${output}")
  endif()
  string(
    JSON values
    GET "${json}"
    targets
    "${name}"
    configured_headers
    "${output}"
    values
  )
  string(JSON kind TYPE "${values}")
  if(NOT kind STREQUAL "OBJECT")
    message(FATAL_ERROR "Native test configured header values must be an object")
  endif()
  file(READ "${root}/${input}" text)
  string(REPLACE "\r\n" "\n" text "${text}")
  string(JSON count LENGTH "${values}")
  if(count GREATER 0)
    math(EXPR last "${count} - 1")
    foreach(index RANGE ${last})
      string(JSON key MEMBER "${values}" ${index})
      string(JSON kind TYPE "${values}" "${key}")
      string(JSON value GET "${values}" "${key}")
      if(NOT key MATCHES "^[A-Za-z0-9_-]+$" OR NOT kind STREQUAL "STRING" OR value MATCHES "[@;\n\r\t]")
        message(FATAL_ERROR "Invalid native test configured header value ${key}")
      endif()
      string(REPLACE "@${key}@" "${value}" text "${text}")
    endforeach()
  endif()
  set(directory "${CMAKE_CURRENT_BINARY_DIR}/native-tests/${name}/configured")
  file(MAKE_DIRECTORY "${directory}")
  # configure_file COPYONLY avoids changing the header's mtime on a no-op configure.
  file(WRITE "${directory}/${output}.tmp" "${text}")
  configure_file("${directory}/${output}.tmp" "${directory}/${output}" COPYONLY)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${root}/${input}")
  target_include_directories(${name} PRIVATE "${directory}")
endfunction()

function(_obs_test_graph_configured_headers json name root)
  string(JSON count ERROR_VARIABLE error LENGTH "${json}" targets "${name}" configured_headers)
  if(NOT error AND count GREATER 0)
    math(EXPR last "${count} - 1")
    foreach(index RANGE ${last})
      string(JSON output MEMBER "${json}" targets "${name}" configured_headers ${index})
      _obs_test_graph_configured_header("${json}" "${name}" "${root}" "${output}")
    endforeach()
  endif()
endfunction()
