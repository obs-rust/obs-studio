# The canonical suite uses build/test/cmocka as its working directory.
# Relocate literals before compilation: shrinking strings in a linked PE would
# invalidate suffix-merged strings and their pointers. Native builds never call this.
function(obs_windows_tier2_relocate directory)
  get_property(_targets DIRECTORY "${directory}" PROPERTY BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_kind "${_target}" TYPE)
    if(NOT _kind MATCHES "^(EXECUTABLE|STATIC_LIBRARY|SHARED_LIBRARY|MODULE_LIBRARY|OBJECT_LIBRARY)$")
      continue()
    endif()
    foreach(_property IN ITEMS COMPILE_DEFINITIONS INTERFACE_COMPILE_DEFINITIONS)
      get_target_property(_definitions "${_target}" "${_property}")
      if(_definitions)
        string(REPLACE "${CMAKE_BINARY_DIR}/test/cmocka/" "" _definitions "${_definitions}")
        string(REPLACE "${CMAKE_BINARY_DIR}" "../.." _definitions "${_definitions}")
        string(REPLACE "${OBS_SOURCE_ROOT}" "../../../source" _definitions "${_definitions}")
        set_property(TARGET "${_target}" PROPERTY "${_property}" "${_definitions}")
      endif()
    endforeach()
    target_compile_options(
      ${_target}
      PRIVATE
        "$<$<COMPILE_LANGUAGE:C,CXX>:-ffile-prefix-map=$ENV{HOME}=toolchain>"
        "$<$<COMPILE_LANGUAGE:C,CXX>:-ffile-prefix-map=${OBS_SOURCE_ROOT}=../../../source>"
        "$<$<COMPILE_LANGUAGE:C,CXX>:-ffile-prefix-map=${CMAKE_BINARY_DIR}=../..>"
    )
    if(CMAKE_SYSTEM_NAME STREQUAL "Windows" AND _kind MATCHES "^(EXECUTABLE|SHARED_LIBRARY|MODULE_LIBRARY)$")
      target_link_options(${_target} PRIVATE "LINKER:/pdbaltpath:$<TARGET_FILE_BASE_NAME:${_target}>.pdb")
    endif()
  endforeach()
  get_property(_children DIRECTORY "${directory}" PROPERTY SUBDIRECTORIES)
  foreach(_child IN LISTS _children)
    obs_windows_tier2_relocate("${_child}")
  endforeach()
endfunction()
