# The Sources dock's name label (OBSSourceLabel) deleted on the main thread
# while its source is destroyed on libobs's destruction thread: a crash on
# quit in signal_handler_disconnect(). Builds the real label on Qt's offscreen
# platform against a real libobs signal handler; linked with --wrap (GNU ld)
# to play the source and count the handler's references, so it needs Qt
# Widgets but no display, no obs_startup() and no frontend build.
#
# OBSSourceLabel.cpp.hpp includes only its group's precompiled header
# (BUILD.md), which brings in the whole frontend.components group. The test
# compiles the file alone through a generated one-line wrapper, with a
# generated stand-in for that header, found first on the include path, that
# includes only the label's part of it.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets)
  if(TARGET Qt6::Widgets)
    set(_sld_gen "${CMAKE_CURRENT_BINARY_DIR}/test_source_label_destroy.gen")
    file(
      CONFIGURE
      OUTPUT "${_sld_gen}/frontend/build/frontend-components.pch.hpp"
      CONTENT "#pragma once\n\n#include \"frontend/components/OBSSourceLabel.hpp\"\n"
    )
    file(
      CONFIGURE
      OUTPUT "${_sld_gen}/OBSSourceLabel.cpp"
      CONTENT "#include \"frontend/components/OBSSourceLabel.cpp.hpp\"\n"
    )
    add_executable(
      test_source_label_destroy
      test_source_label_destroy.cpp
      "${_sld_gen}/OBSSourceLabel.cpp"
      "${CMAKE_SOURCE_DIR}/frontend/components/OBSSourceLabel.hpp"
    )
    set_target_properties(test_source_label_destroy PROPERTIES AUTOMOC ON)
    # BEFORE: the directory's include path already has the source root.
    target_include_directories(test_source_label_destroy BEFORE PRIVATE "${_sld_gen}")
    target_include_directories(
      test_source_label_destroy
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/frontend/components"
    )
    target_link_libraries(test_source_label_destroy PRIVATE OBS::libobs Qt6::Widgets ${CMOCKA_LIBRARIES})
    target_link_options(
      test_source_label_destroy
      PRIVATE
        "LINKER:--wrap=obs_source_get_name"
        "LINKER:--wrap=obs_source_get_signal_handler"
        "LINKER:--wrap=signal_handler_connect_ref"
        "LINKER:--wrap=signal_handler_disconnect"
    )
    add_dependencies(cmocka-tests test_source_label_destroy)
    add_test(test_source_label_destroy ${CMAKE_CURRENT_BINARY_DIR}/test_source_label_destroy)
    unset(_sld_gen)
  endif()
endif()
