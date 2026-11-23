# Both launchers enumerate the same bundle, first from the factory and then
# with a populated cache. Compare the published JSON rather than formatting.
file(MAKE_DIRECTORY "${CACHE_DIR}")
foreach(pass RANGE 1 2)
  foreach(implementation IN ITEMS RUST CPP)
    set(cache "${CACHE_DIR}/${implementation}.json")
    if(pass EQUAL 1)
      file(WRITE "${cache}" "invalid cache\n")
      file(WRITE "${cache}.bak" "invalid backup\n")
    endif()
    execute_process(
      COMMAND "${CMAKE_COMMAND}" -E env "HOME=${HOME_PATH}" "${${implementation}_SCANNER}" "${cache}"
      RESULT_VARIABLE status
      OUTPUT_VARIABLE output
      ERROR_VARIABLE error
    )
    if(NOT status EQUAL 0 OR NOT output MATCHES "(^|\n)VST3_SCAN_DONE\r?\n$")
      message(FATAL_ERROR "${implementation} scanner exit=${status}, output=${output}, error=${error}")
    endif()
    file(READ "${cache}" "${implementation}_cache")
  endforeach()
  string(JSON equivalent EQUAL "${RUST_cache}" "${CPP_cache}")
  if(NOT equivalent)
    message(FATAL_ERROR "Scanner cache differs from C++ on pass ${pass}")
  endif()
endforeach()
