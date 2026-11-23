cmake_minimum_required(VERSION 3.28...3.30)

set(
  scanner_entry_sources
  "$<$<PLATFORM_ID:Windows>:build/obs-vst3-scanner.windows+.hpp.cpp>"
  "$<$<PLATFORM_ID:Darwin,Linux>:build/obs-vst3-scanner.posix+.hpp.cpp>"
)

add_executable(obs-vst3-scanner ${scanner_entry_sources})
if(OBS_LIBOBS_BACKEND STREQUAL HYBRID)
  corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
    CRATES obs-vst3-scanner
    CRATE_TYPES staticlib
    PROFILE release
  )
  obs_serialize_cargo_build(obs-vst3-scanner)
  target_link_libraries(
    obs-vst3-scanner
    PRIVATE
      OBS::libobs
      obs_vst3_scanner-static
      "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>>"
  )

  # Keep the original out of installed builds so the host integration test
  # and parity check can run each implementation in a separate process.
  if(OBS_BUILD_C_REFERENCE OR ENABLE_UNIT_TESTS)
    add_executable(
      obs-vst3-scanner-c-reference
      EXCLUDE_FROM_ALL
      build/obs-vst3-scanner+.hpp.cpp
      ${scanner_entry_sources}
    )
    target_link_libraries(obs-vst3-scanner-c-reference PRIVATE obs-vst3-discovery)
    set_target_properties(obs-vst3-scanner-c-reference PROPERTIES FOLDER plugins/obs-vst3)
    if(OS_WINDOWS)
      obs_msvc_exempt_c4201(obs-vst3-scanner-c-reference)
    endif()
  endif()
else()
  target_sources(obs-vst3-scanner PRIVATE build/obs-vst3-scanner+.hpp.cpp)
  target_link_libraries(obs-vst3-scanner PRIVATE obs-vst3-discovery)
endif()

if(OBS_VST3_SCANNER_NAME)
  set_target_properties(obs-vst3-scanner PROPERTIES OUTPUT_NAME "${OBS_VST3_SCANNER_NAME}")
endif()
if(OS_WINDOWS)
  obs_msvc_exempt_c4201(obs-vst3-scanner)
endif()
set_target_properties_obs(obs-vst3-scanner PROPERTIES FOLDER plugins/obs-vst3)
