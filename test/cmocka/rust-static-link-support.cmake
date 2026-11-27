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

function(_obs_demand_rust_static_support)
  get_directory_property(targets BUILDSYSTEM_TARGETS)
  foreach(target IN LISTS targets)
    get_target_property(type "${target}" TYPE)
    if(NOT type STREQUAL "EXECUTABLE")
      continue()
    endif()
    get_target_property(isolated "${target}" OBS_RUST_STATIC_ISOLATED)
    if(NOT MSVC AND NOT isolated)
      continue()
    endif()
    _obs_uses_rust_static("${target}" "" uses_rust)
    if(uses_rust STREQUAL "0")
      continue()
    endif()
    get_target_property(sources "${target}" SOURCES)
    set(adapters)
    set(fixture_sources)
    foreach(source IN LISTS sources)
      if(source MATCHES "-link-(support|stubs?)\\.c$" OR source MATCHES "^\\$<TARGET_OBJECTS:obs-test-support-[^>]+>$")
        list(APPEND adapters "${source}")
        if(NOT uses_rust STREQUAL "1")
          list(APPEND fixture_sources "$<$<NOT:${uses_rust}>:${source}>")
        endif()
      else()
        list(APPEND fixture_sources "${source}")
      endif()
    endforeach()
    if(NOT adapters)
      continue()
    endif()
    # Preserve the fixture's definitions, source properties and header order.
    # Each adapter remains an independent member; unrelated runtime callbacks
    # must not select Rust objects or DLL imports in an isolated fixture.
    set(local_support "${target}-rust-static-support")
    add_library("${local_support}" STATIC EXCLUDE_FROM_ALL ${adapters})
    set_target_properties("${local_support}" PROPERTIES UNITY_BUILD OFF)
    foreach(
      property
      IN
      ITEMS
        COMPILE_DEFINITIONS
        COMPILE_OPTIONS
        COMPILE_FLAGS
        COMPILE_FEATURES
        INCLUDE_DIRECTORIES
        SYSTEM_INCLUDE_DIRECTORIES
        C_STANDARD
        C_STANDARD_REQUIRED
        C_EXTENSIONS
        MSVC_RUNTIME_LIBRARY
        MSVC_DEBUG_INFORMATION_FORMAT
    )
      get_target_property(value "${target}" "${property}")
      if(NOT value MATCHES "-NOTFOUND$")
        set_property(TARGET "${local_support}" PROPERTY "${property}" "${value}")
      endif()
    endforeach()
    get_target_property(links "${target}" LINK_LIBRARIES)
    target_link_libraries("${local_support}" PRIVATE "$<COMPILE_ONLY:${links}>")
    # Fixture-owned recording adapters precede the generic fallback archive.
    if(uses_rust STREQUAL "1")
      list(INSERT links 1 "${local_support}")
    else()
      list(INSERT links 1 "$<${uses_rust}:${local_support}>")
    endif()
    set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "${links}")
    set_property(TARGET "${target}" PROPERTY SOURCES "${fixture_sources}")
  endforeach()
endfunction()

function(obs_rust_static_consumers support native_owner)
  if(MSVC AND TARGET OBS::libobs AND NOT TARGET obs-rust-static-headers)
    # A standalone Rust executable must use one allocator/logger implementation.
    # Keep the core's header requirements without its competing DLL imports.
    add_library(obs-rust-static-headers INTERFACE)
    foreach(
      property
      IN
      ITEMS INCLUDE_DIRECTORIES SYSTEM_INCLUDE_DIRECTORIES COMPILE_DEFINITIONS COMPILE_OPTIONS COMPILE_FEATURES
    )
      get_target_property(value OBS::libobs INTERFACE_${property})
      if(value)
        set_property(TARGET obs-rust-static-headers PROPERTY INTERFACE_${property} "${value}")
      endif()
    endforeach()
  endif()
  # Real C adapters call back into Rust. Retain that return edge so ELF archive
  # selection searches Rust again after extracting a lazy support member.
  target_link_libraries("${support}" PRIVATE libobs_rust-static)
  set(support_link "${support}")
  if(NOT MSVC AND (CMAKE_C_LINK_GROUP_USING_RESCAN_SUPPORTED OR CMAKE_LINK_GROUP_USING_RESCAN_SUPPORTED))
    # Resolve the complete Rust/C callback component, including newly extracted
    # members that introduce another callback in the opposite direction.
    # Keep the direct support edge visible to existing graph assertions. CMake
    # replaces its non-group occurrence with this same group during generation.
    set(support_link "${support};$<LINK_GROUP:RESCAN,libobs_rust-static,${support}>")
  endif()
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
      if(MSVC AND TARGET obs-rust-static-headers)
        set(provider_links)
        foreach(link IN LISTS links)
          if(link MATCHES "^(OBS::libobs|libobs)$")
            if(uses_rust STREQUAL "1")
              list(APPEND provider_links obs-rust-static-headers)
            else()
              list(APPEND provider_links "$<IF:${uses_rust},obs-rust-static-headers,${link}>")
            endif()
          else()
            list(APPEND provider_links "${link}")
          endif()
        endforeach()
        set(links "${provider_links}")
      endif()
      # Resolve static allocators/parser/logging before OBS's DLL import members.
      # An indirect consumer also needs this explicit first provider.
      if(uses_rust STREQUAL "1")
        if(MSVC)
          list(REMOVE_ITEM links libobs_rust-static)
          set(links "libobs_rust-static;${support};${links}")
        elseif(NOT "libobs_rust-static" IN_LIST links)
          # Preserve intentional ELF/Darwin DSO selection. Make an indirect
          # Rust edge explicit before support so the archive is searched first.
          list(APPEND links libobs_rust-static)
        endif()
        if(MSVC)
          set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "${links}")
        else()
          set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "${links};${support_link}")
        endif()
      else()
        if(MSVC)
          set(links "$<${uses_rust}:libobs_rust-static>;$<${uses_rust}:${support}>;${links}")
        else()
          set(links "${links};$<${uses_rust}:libobs_rust-static>")
        endif()
        if(MSVC)
          set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "${links}")
        else()
          set_property(TARGET "${target}" PROPERTY LINK_LIBRARIES "${links};$<${uses_rust}:${support_link}>")
        endif()
      endif()
    endif()
  endforeach()
  # Run after every legacy deferred fragment has supplied its real adapters.
  # Non-MSVC fixtures opt in when they must exclude a shared implementation.
  cmake_language(DEFER CALL _obs_demand_rust_static_support)
endfunction()

# Shared by the complete cmocka project and independent graph fixture projects.
function(obs_add_rust_static_support root native_owner)
  if(NOT DEFINED OS_WINDOWS)
    set(OS_WINDOWS "${WIN32}")
  endif()
  if(NOT TARGET video_info_query_link_support)
    file(READ "${root}/test/cmocka/build/rust-static-support.json" manifest)
    string(JSON count LENGTH "${manifest}" sources)
    math(EXPR last "${count} - 1")
    set(sources)
    foreach(index RANGE ${last})
      string(JSON source GET "${manifest}" sources ${index})
      list(APPEND sources "${root}/${source}")
    endforeach()
    string(JSON platforms ERROR_VARIABLE missing LENGTH "${manifest}" platform_sources)
    if(NOT missing AND platforms GREATER 0)
      math(EXPR platform_last "${platforms} - 1")
      foreach(index RANGE ${platform_last})
        string(JSON fact MEMBER "${manifest}" platform_sources ${index})
        if(${fact})
          string(JSON count LENGTH "${manifest}" platform_sources "${fact}")
          math(EXPR last "${count} - 1")
          foreach(member RANGE ${last})
            string(JSON source GET "${manifest}" platform_sources "${fact}" ${member})
            list(APPEND sources "${root}/${source}")
          endforeach()
        endif()
      endforeach()
    endif()
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${root}/test/cmocka/build/rust-static-support.json")
    add_library(video_info_query_link_support STATIC ${sources})
    target_include_directories(
      video_info_query_link_support
      PRIVATE "${root}" "${root}/libobs" "${CMAKE_BINARY_DIR}/config"
    )
    if(TARGET OBS::libobs)
      target_include_directories(
        video_info_query_link_support
        PRIVATE "$<TARGET_PROPERTY:OBS::libobs,INTERFACE_INCLUDE_DIRECTORIES>"
      )
    endif()
    target_link_libraries(
      video_info_query_link_support
      PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
    )
  endif()
  obs_rust_static_consumers(video_info_query_link_support "${native_owner}")
endfunction()

function(obs_rust_static_fixture_support root)
  # Real archive replays need the same owners as the full project. Configuration
  # models with an INTERFACE placeholder keep their dependency-free contract.
  if(
    NOT CMAKE_C_COMPILER_LOADED
    OR NOT TARGET libobs_rust-static
    OR TARGET OBS::libobs-native-dependencies
    OR OBS_CROSS_TIER2
  )
    return()
  endif()
  get_target_property(type libobs_rust-static TYPE)
  get_target_property(archive libobs_rust-static IMPORTED_LOCATION)
  if(NOT type STREQUAL "STATIC_LIBRARY" OR NOT EXISTS "${archive}")
    return()
  endif()
  get_directory_property(targets BUILDSYSTEM_TARGETS)
  set(has_consumer FALSE)
  foreach(target IN LISTS targets)
    get_target_property(type "${target}" TYPE)
    if(type STREQUAL "EXECUTABLE")
      _obs_uses_rust_static("${target}" "" uses_rust)
      if(NOT uses_rust STREQUAL "0")
        set(has_consumer TRUE)
      endif()
    endif()
  endforeach()
  if(NOT has_consumer)
    return()
  endif()
  list(APPEND CMAKE_MODULE_PATH "${root}/cmake/finders")
  if(NOT TARGET OBS::caption)
    add_subdirectory("${root}/deps/libcaption" "${CMAKE_BINARY_DIR}/rust-static-caption")
  endif()
  find_package(Uthash REQUIRED)
  find_package(SIMDe REQUIRED)
  find_package(jansson REQUIRED)
  find_package(ZLIB REQUIRED)
  find_package(Threads REQUIRED)
  find_package(
    FFmpeg
    REQUIRED
    COMPONENTS avcodec avformat avutil swscale swresample
  )
  add_library(obs-rust-static-fixture-native INTERFACE)
  add_library(OBS::libobs-native-dependencies ALIAS obs-rust-static-fixture-native)
  target_link_libraries(
    obs-rust-static-fixture-native
    INTERFACE
      OBS::caption
      Uthash::Uthash
      SIMDe::SIMDe
      jansson::jansson
      ZLIB::ZLIB
      Threads::Threads
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMAKE_DL_LIBS}
  )
  if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    find_package(LibUUID REQUIRED)
    find_package(Gio QUIET)
    target_link_libraries(obs-rust-static-fixture-native INTERFACE LibUUID::LibUUID m rt util)
    if(TARGET gio::gio)
      target_link_libraries(obs-rust-static-fixture-native INTERFACE gio::gio)
    endif()
  endif()
  target_link_libraries(libobs_rust-static INTERFACE OBS::libobs-native-dependencies)
  obs_add_rust_static_support("${root}" OBS::libobs-native-dependencies)
endfunction()
