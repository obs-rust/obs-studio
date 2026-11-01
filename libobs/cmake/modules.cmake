# libobs lib-modules (#3256, #3259): each module of libobs/build/modules.d
# compiles in its own OBJECT library, libobs-mod-<module>, the unit the
# module cache (#3262) keys, stores and relinks.
#
# The split runs once, at the end of libobs/CMakeLists.txt, after every source
# list and host file has filled libobs-objects. It moves each unity group off
# libobs-objects into its module's library and gives that library
# libobs-objects' own compile environment (definitions, options, include
# directories, features, linked libraries and language properties), so each
# group compiles with exactly the command it had before. libobs-objects keeps
# its headers, its resource file and its usage requirements (the ported
# groups no target builds take their include directories from it).
#
# Every target that links libobs-objects links each module library too:
# CMake adds an object library's objects only to targets that link it
# directly. The ported groups are not on libobs-objects (only the explicit C
# targets compile them), so this split leaves them where they are.

set(
  _LIBOBS_MODULE_PROPERTIES
  COMPILE_DEFINITIONS
  COMPILE_OPTIONS
  COMPILE_FEATURES
  INCLUDE_DIRECTORIES
  LINK_LIBRARIES
  POSITION_INDEPENDENT_CODE
  C_STANDARD
  C_STANDARD_REQUIRED
  C_EXTENSIONS
  CXX_STANDARD
  CXX_STANDARD_REQUIRED
  CXX_EXTENSIONS
  C_VISIBILITY_PRESET
  CXX_VISIBILITY_PRESET
  OBJC_VISIBILITY_PRESET
  VISIBILITY_INLINES_HIDDEN
  INTERPROCEDURAL_OPTIMIZATION
  MSVC_RUNTIME_LIBRARY
  FOLDER
)

# _libobs_read_modules(<out_names>): the module names, and for each
# _LIBOBS_MODULE_<name>_PATTERNS in the caller's scope.
function(_libobs_read_modules out_names)
  # ci/test_libobs_modules.py runs these functions with cmake -P, where the
  # module dir is passed in and there is no directory to depend on.
  if(CMAKE_SCRIPT_MODE_FILE)
    file(GLOB _module_files "${LIBOBS_MODULES_DIR}/*.json")
  else()
    file(GLOB _module_files CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/build/modules.d/*.json")
  endif()
  if(NOT _module_files)
    message(FATAL_ERROR "No libobs modules in ${CMAKE_CURRENT_SOURCE_DIR}/build/modules.d")
  endif()
  set(_names "")
  foreach(_file IN LISTS _module_files)
    if(NOT CMAKE_SCRIPT_MODE_FILE)
      set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_file}")
    endif()
    get_filename_component(_name "${_file}" NAME_WE)
    file(READ "${_file}" _json)
    string(JSON _count LENGTH "${_json}" groups)
    set(_patterns "")
    if(_count GREATER 0)
      math(EXPR _last "${_count} - 1")
      foreach(_index RANGE ${_last})
        string(JSON _pattern GET "${_json}" groups ${_index})
        # fnmatch to a CMake regex: escape, then * and ? become .* and .
        string(REGEX REPLACE "([][.+^$(){}|\\\\])" "\\\\\\1" _regex "${_pattern}")
        string(REPLACE "*" ".*" _regex "${_regex}")
        string(REPLACE "?" "." _regex "${_regex}")
        list(APPEND _patterns "^${_regex}$")
      endforeach()
    endif()
    set(_LIBOBS_MODULE_${_name}_PATTERNS "${_patterns}" PARENT_SCOPE)
    list(APPEND _names "${_name}")
  endforeach()
  set(${out_names} "${_names}" PARENT_SCOPE)
endfunction()

# _libobs_module_of(<group name> <names> <out>): the module whose pattern
# matches the group; ci/test_build_groups.py guarantees exactly one.
function(_libobs_module_of group names out)
  foreach(_name IN LISTS names)
    foreach(_regex IN LISTS _LIBOBS_MODULE_${_name}_PATTERNS)
      if(group MATCHES "${_regex}")
        set(${out} "${_name}" PARENT_SCOPE)
        return()
      endif()
    endforeach()
  endforeach()
  message(FATAL_ERROR "libobs group ${group} is in no module of build/modules.d (ci/test_build_groups.py)")
endfunction()

# _libobs_compile_only_sourced(<links> <out>): <links> with each target that
# has INTERFACE_SOURCES (OBS::obfuscate on Windows) as $<COMPILE_ONLY:...>.
# libobs-objects links it to compile its group once; a module that linked it
# too would compile that group again and the DLL would define its symbols
# twice. The module still needs its include directories and definitions.
function(_libobs_compile_only_sourced links out)
  set(_result "")
  foreach(_link IN LISTS links)
    if(TARGET "${_link}")
      get_target_property(_sources "${_link}" INTERFACE_SOURCES)
      if(_sources AND NOT _sources MATCHES "-NOTFOUND$")
        list(APPEND _result "$<COMPILE_ONLY:${_link}>")
        continue()
      endif()
    endif()
    list(APPEND _result "${_link}")
  endforeach()
  set(${out} "${_result}" PARENT_SCOPE)
endfunction()

# libobs_split_modules(<consumers...>): move libobs-objects' groups into
# their module libraries and link those into each consumer.
function(libobs_split_modules)
  _libobs_read_modules(_names)

  get_target_property(_sources libobs-objects SOURCES)
  set(_kept "")
  set(_modules "")
  foreach(_source IN LISTS _sources)
    get_filename_component(_file "${_source}" NAME)
    if(NOT _source MATCHES "[<>$]" AND _file MATCHES "^libobs\\.([^+/]+)\\+\\.h\\.(c|m)$")
      _libobs_module_of("${CMAKE_MATCH_1}" "${_names}" _module)
      list(APPEND _LIBOBS_SOURCES_${_module} "${_source}")
      list(APPEND _modules "${_module}")
    else()
      list(APPEND _kept "${_source}")
    endif()
  endforeach()
  list(REMOVE_DUPLICATES _modules)
  list(SORT _modules)

  foreach(_module IN LISTS _modules)
    set(_target "libobs-mod-${_module}")
    add_library(${_target} OBJECT ${_LIBOBS_SOURCES_${_module}})
    foreach(_property IN LISTS _LIBOBS_MODULE_PROPERTIES)
      get_target_property(_value libobs-objects ${_property})
      if(_value MATCHES "-NOTFOUND$")
        continue()
      endif()
      if(_property STREQUAL "LINK_LIBRARIES")
        _libobs_compile_only_sourced("${_value}" _value)
      endif()
      set_property(TARGET ${_target} PROPERTY ${_property} "${_value}")
    endforeach()
    foreach(_consumer IN LISTS ARGN)
      target_link_libraries(${_consumer} PRIVATE ${_target})
    endforeach()
  endforeach()
  set_property(TARGET libobs-objects PROPERTY SOURCES "${_kept}")
  # With every group moved, libobs-objects may compile nothing on a host (it
  # keeps headers, and on Windows its resource file); CMake then cannot infer
  # a linker language. It is an object library and never links itself.
  set_property(TARGET libobs-objects PROPERTY LINKER_LANGUAGE C)
  set_property(GLOBAL PROPERTY LIBOBS_MODULE_TARGETS "${_modules}")
endfunction()
