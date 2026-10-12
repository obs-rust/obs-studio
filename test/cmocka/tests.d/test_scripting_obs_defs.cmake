# Lua and Python scripts can read the constants of libobs/obs-defs.h
# (obsproject/obs-studio#8936). The test generates the Lua and Python SWIG
# wrappers from the real obslua.i and obspython.i and links them into itself,
# so it needs SWIG, LuaJIT and the Python headers, but neither
# ENABLE_SCRIPTING nor an obs core nor a display. Linux and the BSDs only:
# there OBS links Python at build time, as this test does.
if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(SWIG 4 QUIET)
  find_package(Luajit QUIET)
  find_package(Python 3.8 QUIET COMPONENTS Development.Embed)
  if(SWIG_FOUND AND Luajit_FOUND AND Python_Development.Embed_FOUND)
    include(UseSWIG)

    set(_obs_defs_scripting "${CMAKE_SOURCE_DIR}/shared/obs-scripting")
    set(_obs_defs_dir "${CMAKE_CURRENT_BINARY_DIR}/test_scripting_obs_defs_swig")

    # obspython.i includes the obspython.h that obspython/CMakeLists.txt
    # generates; only its PYTHON_LIB name is in it, which the test does not use.
    file(WRITE "${_obs_defs_dir}/obspython.h" "#pragma once\n\n#define PYTHON_LIB \"\"\n")

    set_source_files_properties(
      "${_obs_defs_scripting}/obslua/obslua.i"
      "${_obs_defs_scripting}/obspython/obspython.i"
      PROPERTIES USE_TARGET_INCLUDE_DIRECTORIES TRUE
    )
    # Keep the module's own name, _obspython, rather than the target's.
    set_property(SOURCE "${_obs_defs_scripting}/obspython/obspython.i" PROPERTY SWIG_FLAGS -interface _obspython)

    swig_add_library(
      test_scripting_obs_defs_lua
      LANGUAGE lua
      TYPE STATIC
      SOURCES "${_obs_defs_scripting}/obslua/obslua.i"
      OUTPUT_DIR "${_obs_defs_dir}/lua"
      OUTFILE_DIR "${_obs_defs_dir}/lua"
    )
    target_include_directories(test_scripting_obs_defs_lua PRIVATE "${_obs_defs_scripting}")
    target_compile_definitions(test_scripting_obs_defs_lua PRIVATE SWIG_TYPE_TABLE=obslua SWIG_LUA_INTERPRETER_NO_DEBUG)
    target_compile_options(
      test_scripting_obs_defs_lua
      PRIVATE -Wno-unused-parameter $<$<COMPILE_LANG_AND_ID:C,Clang>:-Wno-shorten-64-to-32>
    )
    target_link_libraries(test_scripting_obs_defs_lua PUBLIC OBS::libobs Luajit::Luajit m)

    swig_add_library(
      test_scripting_obs_defs_python
      LANGUAGE python
      TYPE STATIC
      SOURCES "${_obs_defs_scripting}/obspython/obspython.i"
      OUTPUT_DIR "${_obs_defs_dir}/python"
      OUTFILE_DIR "${_obs_defs_dir}/python"
    )
    target_include_directories(test_scripting_obs_defs_python PRIVATE "${_obs_defs_scripting}" "${_obs_defs_dir}")
    target_compile_definitions(
      test_scripting_obs_defs_python
      PRIVATE SWIG_TYPE_TABLE=obspython Py_ENABLE_SHARED=1 SWIG_PYTHON_INTERPRETER_NO_DEBUG
    )
    target_compile_options(
      test_scripting_obs_defs_python
      PRIVATE -Wno-unused-parameter $<$<COMPILE_LANG_AND_ID:C,Clang>:-Wno-macro-redefined -Wno-unreachable-code>
    )
    target_link_libraries(test_scripting_obs_defs_python PUBLIC OBS::libobs Python::Python m)

    add_executable(test_scripting_obs_defs test_scripting_obs_defs.c)
    target_compile_definitions(test_scripting_obs_defs PRIVATE OBSPYTHON_DIR="${_obs_defs_dir}/python")
    target_include_directories(test_scripting_obs_defs PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(
      test_scripting_obs_defs
      PRIVATE test_scripting_obs_defs_lua test_scripting_obs_defs_python ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_scripting_obs_defs)
    add_test(test_scripting_obs_defs ${CMAKE_CURRENT_BINARY_DIR}/test_scripting_obs_defs)
  endif()
endif()
