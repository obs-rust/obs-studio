# One selection rule for standalone consumers, including interface dependencies.
include_guard(GLOBAL)

function(_obs_uses_rust_static target visited output)
  if(target STREQUAL "libobs_rust-static")
    set(${output} TRUE PARENT_SCOPE)
    return()
  endif()
  if(NOT TARGET "${target}" OR target IN_LIST visited)
    set(${output} FALSE PARENT_SCOPE)
    return()
  endif()
  list(APPEND visited "${target}")
  get_target_property(type "${target}" TYPE)
  # A shared library's implementation is not a standalone Rust consumer.
  if(type MATCHES "^(SHARED|MODULE)_LIBRARY$")
    set(${output} FALSE PARENT_SCOPE)
    return()
  endif()
  get_target_property(links "${target}" INTERFACE_LINK_LIBRARIES)
  get_target_property(private_links "${target}" LINK_LIBRARIES)
  string(REGEX MATCHALL "[A-Za-z0-9_.+-]+(::[A-Za-z0-9_.+-]+)*" dependencies "${links};${private_links}")
  foreach(dependency IN LISTS dependencies)
    _obs_uses_rust_static("${dependency}" "${visited}" uses_rust)
    if(uses_rust)
      set(${output} TRUE PARENT_SCOPE)
      return()
    endif()
  endforeach()
  set(${output} FALSE PARENT_SCOPE)
endfunction()

function(obs_rust_static_consumers support native_owner)
  if(TARGET "${native_owner}")
    # Inherit the owner's full native closure, including host-selected libraries
    # and their transitive interfaces. Never copy GObject/Jansson lists per test.
    target_link_libraries(
      "${support}"
      PRIVATE "$<TARGET_GENEX_EVAL:${native_owner},$<TARGET_PROPERTY:${native_owner},INTERFACE_LINK_LIBRARIES>>"
    )
  endif()
  get_directory_property(targets BUILDSYSTEM_TARGETS)
  foreach(target IN LISTS targets)
    get_target_property(type "${target}" TYPE)
    if(NOT type STREQUAL "EXECUTABLE")
      continue()
    endif()
    _obs_uses_rust_static("${target}" "" uses_rust)
    if(uses_rust)
      get_target_property(links "${target}" LINK_LIBRARIES)
      list(REMOVE_ITEM links libobs_rust-static)
      # Resolve static allocators/parser/logging before OBS's DLL import members.
      # An indirect consumer also needs this explicit first provider.
      set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "libobs_rust-static;${links};${support}")
    endif()
  endforeach()
endfunction()
