# Select the native loader variable without changing the shared fault shim.
if(CMAKE_HOST_APPLE)
  set(preload DYLD_INSERT_LIBRARIES)
else()
  set(preload LD_PRELOAD)
endif()
execute_process(
  COMMAND "${CMAKE_COMMAND}" -E env "${preload}=${FAULT_SHIM}" "${FAULT_TEST}"
  RESULT_VARIABLE result
)
if(NOT result STREQUAL "0")
  message(FATAL_ERROR "config close fault test returned ${result}")
endif()
