cmake_minimum_required(VERSION 3.29)

function(exit_with_status status)
  if(NOT status MATCHES "^[0-9]+$")
    message(FATAL_ERROR "Flatpak build command failed to run: ${status}")
  endif()
  cmake_language(EXIT "${status}")
endfunction()

# cmake -P <script> -- <ninja arguments>
set(ninja_args)
math(EXPR last_arg "${CMAKE_ARGC} - 1")
if(last_arg GREATER_EQUAL 4)
  foreach(index RANGE 4 ${last_arg})
    string(REPLACE ";" "\\;" arg "${CMAKE_ARGV${index}}")
    list(APPEND ninja_args "${arg}")
  endforeach()
endif()

# Builder passes -j first for the module build. Ninja probes and the separate
# install invocation run directly, with zccache disabled by default.
set(module "$ENV{FLATPAK_BUILDER_BUILDDIR}")
if(NOT CMAKE_ARGV4 MATCHES "^-j[0-9]+" OR NOT CMAKE_CURRENT_BINARY_DIR STREQUAL "${module}/_flatpak_build")
  execute_process(COMMAND /usr/bin/ninja ${ninja_args} RESULT_VARIABLE result)
  exit_with_status("${result}")
endif()

set(zccache "${module}/.flatpak-zccache/zccache")
if(IS_WRITABLE "/run/ccache")
  set(cache "/run/ccache/obs-zccache")
else()
  set(cache "${module}/.flatpak-zccache-state")
endif()
file(MAKE_DIRECTORY "${cache}")
set(ENV{ZCCACHE_CACHE_DIR} "${cache}/state")
unset(ENV{ZCCACHE_DISABLE})

set(session_args --stats)
if("$ENV{OBS_FLATPAK_ZCCACHE_JOURNAL}" STREQUAL "1")
  list(APPEND session_args --journal "${module}/flatpak-compiler.jsonl")
endif()
execute_process(COMMAND "${zccache}" session-start ${session_args} RESULT_VARIABLE start_result OUTPUT_VARIABLE session)
string(JSON session_id ERROR_VARIABLE session_error GET "${session}" session_id)
if(NOT start_result STREQUAL "0" OR NOT session_error STREQUAL "NOTFOUND" OR session_id STREQUAL "")
  execute_process(COMMAND "${zccache}" stop)
  message(FATAL_ERROR "Flatpak zccache failed to start a session (exit ${start_result}, JSON status ${session_error})")
endif()
set(ENV{ZCCACHE_SESSION_ID} "${session_id}")

execute_process(COMMAND /usr/bin/ninja ${ninja_args} RESULT_VARIABLE build_result)
execute_process(
  COMMAND "${zccache}" session-end --json "${session_id}"
  RESULT_VARIABLE stats_result
  OUTPUT_FILE "${cache}/session.pending"
)
execute_process(COMMAND "${zccache}" stop RESULT_VARIABLE stop_result)
foreach(result IN ITEMS "${build_result}" "${stats_result}" "${stop_result}")
  if(NOT result STREQUAL "0")
    exit_with_status("${result}")
  endif()
endforeach()
file(RENAME "${cache}/session.pending" "${cache}/session.json")
