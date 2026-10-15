# obs-websocket's CreateProfile and RemoveProfile returned before the profile
# list had changed (obsproject/obs-websocket#1283): the frontend API's profile
# calls did not wait for the UI thread. Builds the helper the API uses
# (frontend/utility/frontend-api-call.hpp) against QtCore only; no display.
find_package(Qt6 QUIET COMPONENTS Core)
if(TARGET Qt6::Core)
  add_executable(test_frontend_api_call test_frontend_api_call.cpp)
  target_include_directories(
    test_frontend_api_call
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
  )
  target_link_libraries(test_frontend_api_call PRIVATE Qt6::Core ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_frontend_api_call)
  add_test(test_frontend_api_call ${CMAKE_CURRENT_BINARY_DIR}/test_frontend_api_call)
  set_tests_properties(test_frontend_api_call PROPERTIES TIMEOUT 60)
  cmocka_qt_runtime(test_frontend_api_call Qt6::Core)
endif()
