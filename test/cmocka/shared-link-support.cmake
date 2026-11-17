# Run after every tests.d deferred hook. Keep each existing translation unit
# and its eager object inclusion; demand-linked support archives stay untouched.
include_guard(GLOBAL)

# Equal raw target settings and equal ordered dependency closures are a
# conservative compile-context key. Never infer equality from a source's name.
# Consumer-sensitive generator expressions cannot be shared by this rule.
function(_obs_support_dependency_context target output)
  set(context)
  get_target_property(pending "${target}" LINK_LIBRARIES)
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
    )
      get_target_property(value "${dependency}" "${property}")
      string(APPEND context "${dependency}/${property}=[${value}]\n")
      if(property STREQUAL "INTERFACE_LINK_LIBRARIES" AND value)
        # Follow both sides of conditional link expressions conservatively.
        string(REGEX MATCHALL "[A-Za-z0-9_:.-]+" candidates "${value}")
        list(APPEND pending ${candidates})
      endif()
    endforeach()
  endwhile()
  set(${output} "${context}" PARENT_SCOPE)
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
    foreach(property IN LISTS properties)
      get_property(is_set TARGET "${target}" PROPERTY "${property}" SET)
      get_target_property(value "${target}" "${property}")
      string(APPEND context "${property}/${is_set}=[${value}]\n")
    endforeach()
    # These expressions can depend on the consuming target, its name/type or
    # link language. Leave them in place rather than silently changing context.
    if(context MATCHES "\\$<(TARGET_|GENEX_EVAL|LINK_LANGUAGE|LINK_LANG_AND_ID)")
      continue()
    endif()
    string(SHA256 context_key "${context}")
    set(owner_${context_key} "${target}")
    get_target_property(sources "${target}" SOURCES)
    foreach(source IN LISTS sources)
      if(NOT source MATCHES "-link-support\\.c$" OR source MATCHES "\\$<")
        continue()
      endif()
      set(source_context)
      foreach(property IN ITEMS COMPILE_DEFINITIONS COMPILE_OPTIONS COMPILE_FLAGS INCLUDE_DIRECTORIES)
        get_source_file_property(value "${source}" "${property}")
        string(APPEND source_context "${value}")
      endforeach()
      if(source_context MATCHES "\\$<(TARGET_|GENEX_EVAL|LINK_LANGUAGE|LINK_LANG_AND_ID)")
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
    set_target_properties("${support}" PROPERTIES UNITY_BUILD OFF)
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
