# Run after every tests.d deferred hook. Keep each existing translation unit
# and its eager object inclusion; demand-linked support archives stay untouched.
include_guard(GLOBAL)

# Equal raw target settings and equal ordered dependency closures are a
# conservative compile-context key. Never infer equality from a source's name.
# Consumer-sensitive generator expressions cannot be shared by this rule.
function(_obs_support_dependency_context target output)
  set(context)
  get_target_property(links "${target}" LINK_LIBRARIES)
  string(REGEX MATCHALL "[A-Za-z0-9_.+-]+(::[A-Za-z0-9_.+-]+)*" pending "${links}")
  set(visited)
  while(pending)
    list(POP_FRONT pending dependency)
    if(NOT TARGET "${dependency}" OR dependency IN_LIST visited)
      continue()
    endif()
    list(APPEND visited "${dependency}")
    foreach(
      property
      IN
      ITEMS
        INTERFACE_COMPILE_DEFINITIONS
        INTERFACE_COMPILE_OPTIONS
        INTERFACE_COMPILE_FEATURES
        INTERFACE_INCLUDE_DIRECTORIES
        INTERFACE_SYSTEM_INCLUDE_DIRECTORIES
        INTERFACE_LINK_LIBRARIES
        INTERFACE_POSITION_INDEPENDENT_CODE
        INTERFACE_LINK_OPTIONS
        INTERFACE_LINK_DIRECTORIES
        INTERFACE_SOURCES
        INTERFACE_PRECOMPILE_HEADERS
    )
      get_target_property(value "${dependency}" "${property}")
      if(value AND property MATCHES "^INTERFACE_(SOURCES|PRECOMPILE_HEADERS)$")
        set(${output} "unshareable" PARENT_SCOPE)
        return()
      endif()
      string(APPEND context "${dependency}/${property}=[${value}]\n")
      if(property STREQUAL "INTERFACE_LINK_LIBRARIES" AND value)
        # Follow both sides of conditional link expressions conservatively.
        string(REGEX MATCHALL "[A-Za-z0-9_.+-]+(::[A-Za-z0-9_.+-]+)*" candidates "${value}")
        list(APPEND pending ${candidates})
      endif()
    endforeach()
  endwhile()
  set(${output} "${context}" PARENT_SCOPE)
endfunction()

# Explicit target/property queries are stable only if the queried property's
# expressions are stable too. Inspect those recursively before allowing them.
function(_obs_support_context_safe context output)
  set(visited)
  while(TRUE)
    if(
      context
        MATCHES
        "\\$<(TARGET_NAME|TARGET_POLICY|TARGET_OBJECTS|GENEX_EVAL|TARGET_GENEX_EVAL|LINK_LANGUAGE|LINK_LANG_AND_ID)(:|>)"
    )
      set(${output} FALSE PARENT_SCOPE)
      return()
    endif()
    string(REGEX MATCHALL "\\$<TARGET_PROPERTY:[^,<>$]+,[^,<>$]+>" queries "${context}")
    set(nested)
    foreach(query IN LISTS queries)
      string(REPLACE "${query}" "" context "${context}")
      if(query IN_LIST visited)
        continue()
      endif()
      list(APPEND visited "${query}")
      string(REGEX REPLACE "^\\$<TARGET_PROPERTY:([^,]+),([^>]+)>$" "\\1" target "${query}")
      string(REGEX REPLACE "^\\$<TARGET_PROPERTY:([^,]+),([^>]+)>$" "\\2" property "${query}")
      if(NOT TARGET "${target}")
        set(${output} FALSE PARENT_SCOPE)
        return()
      endif()
      get_target_property(value "${target}" "${property}")
      string(APPEND nested "${value}\n")
    endforeach()
    # Dynamic or single-argument property queries depend on the consumer.
    if(context MATCHES "\\$<TARGET_PROPERTY:")
      set(${output} FALSE PARENT_SCOPE)
      return()
    endif()
    if(NOT queries)
      break()
    endif()
    set(context "${nested}")
  endwhile()
  set(${output} TRUE PARENT_SCOPE)
endfunction()

function(obs_share_test_link_support)
  get_directory_property(targets BUILDSYSTEM_TARGETS)
  set(
    properties
    COMPILE_DEFINITIONS
    COMPILE_OPTIONS
    COMPILE_FLAGS
    COMPILE_FEATURES
    INCLUDE_DIRECTORIES
    SYSTEM_INCLUDE_DIRECTORIES
    LINK_LIBRARIES
    LINK_OPTIONS
    LINK_FLAGS
    C_STANDARD
    C_STANDARD_REQUIRED
    C_EXTENSIONS
    C_VISIBILITY_PRESET
    POSITION_INDEPENDENT_CODE
    VISIBILITY_INLINES_HIDDEN
    INTERPROCEDURAL_OPTIMIZATION
    OSX_ARCHITECTURES
    COMPILE_WARNING_AS_ERROR
    NO_SYSTEM_FROM_IMPORTED
    MSVC_RUNTIME_LIBRARY
    MSVC_DEBUG_INFORMATION_FORMAT
    C_COMPILER_LAUNCHER
    DISABLE_PRECOMPILE_HEADERS
    MANUALLY_ADDED_DEPENDENCIES
  )
  foreach(
    config
    IN
    ITEMS Debug Release RelWithDebInfo MinSizeRel ${CMAKE_CONFIGURATION_TYPES}
  )
    string(TOUPPER "${config}" config)
    list(
      APPEND properties
      "COMPILE_DEFINITIONS_${config}"
      "LINK_FLAGS_${config}"
      "INTERPROCEDURAL_OPTIMIZATION_${config}"
      "OSX_ARCHITECTURES_${config}"
    )
  endforeach()
  list(REMOVE_DUPLICATES properties)
  set(entries)
  foreach(target IN LISTS targets)
    get_target_property(type "${target}" TYPE)
    get_target_property(links "${target}" LINK_LIBRARIES)
    if(NOT type STREQUAL "EXECUTABLE" OR NOT "libobs_rust-static" IN_LIST links)
      continue()
    endif()
    # Moving a member out of a unity/PCH target could change macro scope or
    # --wrap undefined-reference boundaries. Such targets keep their old build.
    get_target_property(unity "${target}" UNITY_BUILD)
    get_target_property(pch "${target}" PRECOMPILE_HEADERS)
    get_target_property(reuse_pch "${target}" PRECOMPILE_HEADERS_REUSE_FROM)
    get_target_property(exports "${target}" ENABLE_EXPORTS)
    get_target_property(automoc "${target}" AUTOMOC)
    get_target_property(autouic "${target}" AUTOUIC)
    get_target_property(autorcc "${target}" AUTORCC)
    if(
      unity
      OR pch
      OR reuse_pch
      OR exports
      OR automoc
      OR autouic
      OR autorcc
    )
      continue()
    endif()
    _obs_support_dependency_context("${target}" context)
    if(context STREQUAL "unshareable")
      continue()
    endif()
    foreach(property IN LISTS properties)
      get_property(is_set TARGET "${target}" PROPERTY "${property}" SET)
      get_target_property(value "${target}" "${property}")
      string(APPEND context "${property}/${is_set}=[${value}]\n")
    endforeach()
    _obs_support_context_safe("${context}" safe)
    if(NOT safe)
      continue()
    endif()
    string(SHA256 context_key "${context}")
    set(owner_${context_key} "${target}")
    get_target_property(sources "${target}" SOURCES)
    foreach(source IN LISTS sources)
      if(NOT source MATCHES "-(link-(support|stubs?)|fields)\\.c$" OR source MATCHES "\\$<")
        continue()
      endif()
      get_source_file_property(language "${source}" LANGUAGE)
      if(language AND NOT language STREQUAL "C")
        continue()
      endif()
      set(source_context)
      set(source_properties COMPILE_DEFINITIONS COMPILE_OPTIONS COMPILE_FLAGS INCLUDE_DIRECTORIES)
      foreach(property IN LISTS properties)
        if(property MATCHES "^COMPILE_DEFINITIONS_")
          list(APPEND source_properties "${property}")
        endif()
      endforeach()
      foreach(property IN LISTS source_properties)
        get_source_file_property(value "${source}" "${property}")
        string(APPEND source_context "${value}")
      endforeach()
      if(
        source_context
          MATCHES
          "\\$<(TARGET_NAME|TARGET_POLICY|TARGET_OBJECTS|GENEX_EVAL|TARGET_GENEX_EVAL|LINK_LANGUAGE|LINK_LANG_AND_ID)(:|>)|\\$<TARGET_PROPERTY:[^,>]+>"
      )
        continue()
      endif()
      # Source properties are directory-scoped and retained on the same path.
      # A generated source keeps its original custom-command dependency.
      string(SHA256 entry "${context_key}/${source}")
      if(NOT DEFINED source_${entry})
        list(APPEND entries "${entry}")
        set(source_${entry} "${source}")
        set(context_${entry} "${context_key}")
      endif()
      list(APPEND consumers_${entry} "${target}")
    endforeach()
  endforeach()

  # One object target per shared source/context keeps each translation unit
  # separate, including --wrap's undefined-reference boundaries.
  foreach(entry IN LISTS entries)
    list(REMOVE_DUPLICATES consumers_${entry})
    list(LENGTH consumers_${entry} count)
    if(count LESS 2)
      continue()
    endif()
    set(support "obs-test-support-${entry}")
    set(owner "${owner_${context_${entry}}}")
    add_library("${support}" OBJECT "${source_${entry}}")
    # Explicitly disable automatic batching inherited from the directory.
    set_target_properties(
      "${support}"
      PROPERTIES UNITY_BUILD OFF AUTOMOC OFF AUTOUIC OFF AUTORCC OFF
    )
    foreach(property IN LISTS properties)
      if(property MATCHES "^(LINK_OPTIONS|LINK_FLAGS.*|MANUALLY_ADDED_DEPENDENCIES)$")
        continue()
      endif()
      get_property(is_set TARGET "${owner}" PROPERTY "${property}" SET)
      if(is_set)
        get_target_property(value "${owner}" "${property}")
        set_property(TARGET "${support}" PROPERTY "${property}" "${value}")
      else()
        set_property(TARGET "${support}" PROPERTY "${property}")
      endif()
    endforeach()
    get_target_property(dependencies "${owner}" MANUALLY_ADDED_DEPENDENCIES)
    if(dependencies)
      add_dependencies("${support}" ${dependencies})
    endif()
    foreach(consumer IN LISTS consumers_${entry})
      get_target_property(sources "${consumer}" SOURCES)
      set(replaced)
      foreach(source IN LISTS sources)
        if(source STREQUAL source_${entry})
          list(APPEND replaced "$<TARGET_OBJECTS:${support}>")
        else()
          list(APPEND replaced "${source}")
        endif()
      endforeach()
      set_property(TARGET "${consumer}" PROPERTY SOURCES "${replaced}")
    endforeach()
  endforeach()
endfunction()
