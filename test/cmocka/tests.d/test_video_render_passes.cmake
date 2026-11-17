# Both executables keep the C assertions unchanged. The reference compiles the
# retained C orchestration; _rust reaches the actual Rust ABI via the real adapter.
set(_video_passes_root "${CMAKE_CURRENT_LIST_DIR}/../../..")
set(_video_passes_source "${_video_passes_root}/libobs/obs-video-render-passes.c.h")
set(_video_passes_adapter "${_video_passes_root}/libobs/obs-video-render-passes-adapter.c.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_video_passes_source}" "${_video_passes_adapter}")
file(READ "${_video_passes_source}" _video_passes_bodies)
file(READ "${_video_passes_adapter}" _video_passes_adapter_text)
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _video_passes_bodies "${_video_passes_bodies}")
string(REPLACE "#include \"libobs/build/libobs.pch.h\"" "" _video_passes_adapter_text "${_video_passes_adapter_text}")

set(_video_passes_variants reference)
if(TARGET libobs_rust-static)
  list(APPEND _video_passes_variants rust)
endif()
foreach(_variant IN LISTS _video_passes_variants)
  set(_target test_video_render_passes)
  set(_bodies "${_video_passes_bodies}")
  if(_variant STREQUAL rust)
    string(APPEND _target "_rust")
    set(_bodies "")
  endif()
  set(_include_dir "${CMAKE_CURRENT_BINARY_DIR}/video-render-passes-${_variant}")
  file(MAKE_DIRECTORY "${_include_dir}")
  file(WRITE "${_include_dir}/video-render-passes-under-test.h" "${_bodies}\n${_video_passes_adapter_text}")
  add_executable(${_target} "${CMAKE_CURRENT_LIST_DIR}/../test_video_render_passes.c")
  target_include_directories(${_target} PRIVATE "${_include_dir}" ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(${_target} PRIVATE ${CMOCKA_LIBRARIES})
  if(_variant STREQUAL rust)
    # Deferred Rust-static link support includes private libobs headers, which
    # consume the generated obsconfig.h in the full Tier 2 build.
    target_include_directories(
      ${_target}
      PRIVATE "${CMAKE_BINARY_DIR}/config" "${_video_passes_root}" "${_video_passes_root}/libobs"
    )
    # The deferred FFmpeg adapter also needs blog on MSVC, where its unused
    # warning function is retained. Supply the real variadic logging half.
    target_sources(${_target} PRIVATE "${_video_passes_root}/libobs/util/base-variadic.c.h")
    target_link_libraries(${_target} PRIVATE Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>")
    if(TARGET gio::gio)
      find_package(PkgConfig REQUIRED)
      pkg_check_modules(_video_passes_gio REQUIRED IMPORTED_TARGET gio-2.0 glib-2.0 gobject-2.0)
      target_link_libraries(${_target} PRIVATE PkgConfig::_video_passes_gio)
    endif()
    if(TARGET LibUUID::LibUUID)
      target_link_libraries(${_target} PRIVATE LibUUID::LibUUID)
    endif()
    if(WIN32)
      target_link_libraries(${_target} PRIVATE Rpcrt4 winmm)
    endif()
    if(TARGET libobs_rust-static)
      # MSVC retains the obs-util archive member containing filename formatting.
      # Keep its unused video-info provider separate so fixture definitions win.
      add_library(
        video_render_passes_platform_link_support
        STATIC
        "${CMAKE_CURRENT_SOURCE_DIR}/source-enabled-setter-video-link-stub.c"
      )
      target_link_libraries(${_target} PRIVATE libobs_rust-static video_render_passes_platform_link_support)
    endif()
    # Drop unrelated Rust archive sections, whose private C adapters belong to
    # other consumers. The render entry points have only callback dependencies.
    if(MSVC)
      target_link_options(${_target} PRIVATE /OPT:REF)
    elseif(APPLE)
      target_link_options(${_target} PRIVATE -Wl,-dead_strip)
    else()
      target_link_options(${_target} PRIVATE -Wl,--gc-sections)
    endif()
  endif()
  add_dependencies(cmocka-tests ${_target})
  add_test(NAME ${_target} COMMAND ${_target})
  obs_test_sanitize(${_target} "address,undefined")
endforeach()
