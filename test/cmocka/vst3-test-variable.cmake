foreach(implementation IN ITEMS CPP RUST)
  execute_process(
    COMMAND "${${implementation}_HOST}" "${FIXTURE}"
    RESULT_VARIABLE status
    OUTPUT_VARIABLE output
    ERROR_VARIABLE error
  )
  if(NOT status EQUAL 0)
    message(FATAL_ERROR "${implementation} VST3 host exit=${status}\n${output}\n${error}")
  endif()
  string(REGEX MATCHALL "VST3_PARITY [0-9a-f]+" "${implementation}_results" "${output}")
  list(LENGTH "${implementation}_results" count)
  if(NOT count EQUAL 2)
    message(FATAL_ERROR "${implementation} VST3 host did not complete both activation cycles")
  endif()
endforeach()
if(NOT CPP_results STREQUAL RUST_results)
  message(FATAL_ERROR "Variable-block samples differ between C++ and Rust\n${CPP_results}\n${RUST_results}")
endif()
