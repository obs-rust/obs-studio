# Volume meter repaints stop when no meter has anything new to draw
# (obs-rust/obs-studio#1281). Compiles the frontend's header-only ticker and
# runs it with fake meters on Qt's offscreen platform, so it needs Qt Core but
# no display and no frontend build.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
  if(TARGET Qt6::Core)
    add_executable(test_volume_meter_ticker test_volume_meter_ticker.cpp)
    target_include_directories(
      test_volume_meter_ticker
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/components"
    )
    target_link_libraries(test_volume_meter_ticker PRIVATE Qt6::Core ${CMOCKA_LIBRARIES})
    if(NOT MSVC)
      target_link_libraries(test_volume_meter_ticker PRIVATE Threads::Threads)
    endif()
    add_dependencies(cmocka-tests test_volume_meter_ticker)
    add_test(test_volume_meter_ticker ${CMAKE_CURRENT_BINARY_DIR}/test_volume_meter_ticker)
  endif()
endif()
