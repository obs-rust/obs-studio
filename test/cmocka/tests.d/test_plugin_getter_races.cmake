# Private plugin fields are reached through the production implementation,
# with unused callbacks discarded. These no-display tests also run under TSan.
if(OS_LINUX AND TARGET obs-filters AND TARGET image-source AND TARGET obs-transitions AND TARGET linux-capture)
  foreach(
    plugin
    IN
    ITEMS SCROLL SLIDESHOW SLIDESHOW_MK2 STINGER XSHM
  )
    string(TOLOWER "${plugin}" case)
    set(test_name "test_plugin_getter_${case}")
    add_executable(${test_name} test_plugin_getter_races.c)
    target_compile_definitions(${test_name} PRIVATE GETTER_${plugin} _GNU_SOURCE)
    target_compile_options(${test_name} PRIVATE -ffunction-sections -fdata-sections -UNDEBUG)
    target_link_options(${test_name} PRIVATE -Wl,--gc-sections)
    target_include_directories(
      ${test_name}
      PRIVATE
        "${CMAKE_SOURCE_DIR}"
        "${CMAKE_SOURCE_DIR}/plugins/image-source"
        "${CMAKE_SOURCE_DIR}/plugins/obs-transitions"
        "${CMAKE_SOURCE_DIR}/plugins/linux-capture"
    )
    target_link_libraries(
      ${test_name}
      PRIVATE
        OBS::libobs
        m
        OBS::glad
        X11::X11
        X11::XCB
        XCB::XCB
        XCB::XFIXES
        XCB::RANDR
        XCB::SHM
        XCB::XINERAMA
        XCB::COMPOSITE
    )
    add_dependencies(cmocka-tests ${test_name})
    add_test(NAME ${test_name} COMMAND ${test_name})
    set_tests_properties(${test_name} PROPERTIES TIMEOUT 30)
  endforeach()
endif()
