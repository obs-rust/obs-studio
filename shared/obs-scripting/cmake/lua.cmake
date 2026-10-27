option(ENABLE_SCRIPTING_LUA "Enable Lua scripting support" ON)

if(ENABLE_SCRIPTING_LUA)
  add_subdirectory(obslua)

  find_package(Luajit REQUIRED)

  add_custom_command(
    OUTPUT swig/swigluarun.h
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    PRE_BUILD
    COMMAND ${CMAKE_COMMAND} -E make_directory swig
    COMMAND ${CMAKE_COMMAND} -E env "SWIG_LIB=${SWIG_DIR}" ${SWIG_EXECUTABLE} -lua -external-runtime swig/swigluarun.h
    COMMENT "obs-scripting - generating Luajit SWIG interface headers"
  )

  set_source_files_properties(swig/swigluarun.h PROPERTIES GENERATED TRUE)

  target_sources(
    obs-scripting
    PRIVATE
      $<$<BOOL:${ENABLE_FRONTEND}>:build/obs-scripting.lua-frontend+.h.c>
      build/obs-scripting.lua+.h.c
      $<$<PLATFORM_ID:Darwin>:build/obs-scripting.lua-host-macos+.h.c>
      $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-scripting.lua-host-posix+.h.c>
      $<$<PLATFORM_ID:Windows>:build/obs-scripting.lua-host-windows+.h.c>
      obs-scripting-lua.h
      obs-scripting-lua-host.h
      swig/swigluarun.h
  )

  # The SWIG-generated swigluarun.h casts const away (-Wcast-qual, obs-rust/obs-studio#516),
  # and the generated glue declares its entry points with no prototype in scope
  # (-Wmissing-prototypes, obs-rust/obs-studio#2703). Neither file is ours, so
  # this target opts out of those diagnostics instead of the global flag list.
  target_compile_options(
    obs-scripting
    PRIVATE
      $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-cast-qual;-Wno-missing-prototypes;-Wno-strict-prototypes>
  )

  target_compile_definitions(obs-scripting PUBLIC LUAJIT_FOUND)
  # The Lua C module suffix the startup scripts splice in (obs-scripting-lua-host.h).
  target_compile_definitions(obs-scripting PRIVATE OBS_LUA_SO_EXT="$<IF:$<PLATFORM_ID:Windows>,dll,so>")

  set_source_files_properties(
    build/obs-scripting.lua+.h.c
    PROPERTIES COMPILE_OPTIONS $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-error=shorten-64-to-32>
  )
  # The SWIG runtime obs-scripting-lua.h includes: unused parameters and
  # locals, narrowing conversions (MSVC C4100, C4189, C4244, C4267).
  set_property(
    SOURCE build/obs-scripting.lua+.h.c build/obs-scripting.lua-frontend+.h.c
    APPEND
    PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4100;/wd4189;/wd4244;/wd4267>"
  )

  target_link_libraries(obs-scripting PRIVATE Luajit::Luajit)
else()
  target_disable_feature(obs-scripting "Lua scripting support")
endif()
