# create_instance is hidden in libobs. Compile its current production C body
# verbatim under a test-local name. Public ref/release use actual-library symbols.
set(_packet_ownership_source "${CMAKE_SOURCE_DIR}/libobs/obs-encoder-packet-ownership.c.h")
if(NOT EXISTS "${_packet_ownership_source}")
  set(_packet_ownership_source "${CMAKE_SOURCE_DIR}/libobs/obs-encoder.c.h")
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_packet_ownership_source}")
file(READ "${_packet_ownership_source}" _packet_ownership_text)
string(FIND "${_packet_ownership_text}" "void obs_encoder_packet_create_instance(" _packet_ownership_start)
if(_packet_ownership_start LESS 0)
  message(FATAL_ERROR "Cannot locate obs_encoder_packet_create_instance")
endif()
string(SUBSTRING "${_packet_ownership_text}" ${_packet_ownership_start} -1 _packet_ownership_tail)
string(FIND "${_packet_ownership_tail}" "\n}\n" _packet_ownership_end)
if(_packet_ownership_end LESS 0)
  message(FATAL_ERROR "Cannot locate end of obs_encoder_packet_create_instance")
endif()
math(EXPR _packet_ownership_length "${_packet_ownership_end} + 3")
string(SUBSTRING "${_packet_ownership_tail}" 0 ${_packet_ownership_length} _packet_ownership_body)
string(
  REPLACE
  "obs_encoder_packet_create_instance"
  "packet_create_under_test"
  _packet_ownership_body
  "${_packet_ownership_body}"
)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/encoder-packet-create-under-test.h" "static ${_packet_ownership_body}")

add_executable(test_encoder_packet_ownership test_encoder_packet_ownership.c)
target_include_directories(test_encoder_packet_ownership PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(test_encoder_packet_ownership PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_encoder_packet_ownership)
add_test(NAME test_encoder_packet_ownership COMMAND test_encoder_packet_ownership)
set_tests_properties(test_encoder_packet_ownership PROPERTIES TIMEOUT 10)
