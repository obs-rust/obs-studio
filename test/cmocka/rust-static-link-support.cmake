# One selection rule for standalone consumers, including interface dependencies.
include_guard(GLOBAL)

function(_obs_uses_rust_static target visited output)
  if(target STREQUAL "libobs_rust-static")
    set(${output} 1 PARENT_SCOPE)
    return()
  endif()
  if(NOT TARGET "${target}" OR target IN_LIST visited)
    set(${output} 0 PARENT_SCOPE)
    return()
  endif()
  list(APPEND visited "${target}")
  get_target_property(type "${target}" TYPE)
  # A shared library's implementation is not a standalone Rust consumer.
  if(type MATCHES "^(SHARED|MODULE)_LIBRARY$")
    set(${output} 0 PARENT_SCOPE)
    return()
  endif()
  get_target_property(links "${target}" INTERFACE_LINK_LIBRARIES)
  get_target_property(private_links "${target}" LINK_LIBRARIES)
  set(links "${links};${private_links}")
  string(REPLACE ";" "$<SEMICOLON>" condition_links "${links}")
  string(REGEX MATCHALL "[A-Za-z0-9_.+-]+(::[A-Za-z0-9_.+-]+)*" dependencies "${links}")
  list(REMOVE_DUPLICATES dependencies)
  set(conditions)
  foreach(dependency IN LISTS dependencies)
    _obs_uses_rust_static("${dependency}" "${visited}" uses_rust)
    if(NOT uses_rust STREQUAL "0")
      if(dependency IN_LIST links)
        if(uses_rust STREQUAL "1")
          set(${output} 1 PARENT_SCOPE)
          return()
        endif()
        list(APPEND conditions "${uses_rust}")
      else()
        # Evaluate conditional edges with their original owner and links, not
        # the subsequently rewritten consumer property (which would recurse).
        list(
          APPEND conditions
          "$<AND:$<IN_LIST:${dependency},$<TARGET_GENEX_EVAL:${target},${condition_links}>>,${uses_rust}>"
        )
      endif()
    endif()
  endforeach()
  if(conditions)
    list(JOIN conditions "," condition)
    set(${output} "$<OR:${condition}>" PARENT_SCOPE)
  else()
    set(${output} 0 PARENT_SCOPE)
  endif()
endfunction()

function(obs_rust_static_consumers support native_owner)
  if(TARGET "${native_owner}")
    # Inherit the owner's full native closure, including host-selected libraries
    # and their transitive interfaces. Never copy GObject/Jansson lists per test.
    target_link_libraries("${support}" PRIVATE "${native_owner}")
  endif()
  get_directory_property(targets BUILDSYSTEM_TARGETS)
  foreach(target IN LISTS targets)
    get_target_property(type "${target}" TYPE)
    if(NOT type STREQUAL "EXECUTABLE")
      continue()
    endif()
    _obs_uses_rust_static("${target}" "" uses_rust)
    if(NOT uses_rust STREQUAL "0")
      get_target_property(links "${target}" LINK_LIBRARIES)
      list(REMOVE_ITEM links libobs_rust-static)
      # Resolve static allocators/parser/logging before OBS's DLL import members.
      # An indirect consumer also needs this explicit first provider.
      if(uses_rust STREQUAL "1")
        set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "libobs_rust-static;${links};${support}")
      else()
        set_property(
          TARGET "${target}"
          PROPERTY LINK_LIBRARIES "$<${uses_rust}:libobs_rust-static>;${links};$<${uses_rust}:${support}>"
        )
      endif()
    endif()
  endforeach()
endfunction()
