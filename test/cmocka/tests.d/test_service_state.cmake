# Hidden C-source characterization only; the public lifecycle suite tests the swap.
file(READ "${CMAKE_CURRENT_LIST_DIR}/../../../libobs/obs-service-state.c.h" _service_state_source)
string(FIND "${_service_state_source}" "void obs_service_set_destroy(" _service_state_start)
string(FIND "${_service_state_source}" "#undef get_weak" _service_state_end)
math(EXPR _service_state_length "${_service_state_end} - ${_service_state_start}")
string(SUBSTRING "${_service_state_source}" ${_service_state_start} ${_service_state_length} _service_state_body)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/service-state-characterization.inc" "${_service_state_body}")
# Generated include interface used by ../build/test-graph.d.
include("${CMAKE_CURRENT_LIST_DIR}/../characterization-headers.cmake")
