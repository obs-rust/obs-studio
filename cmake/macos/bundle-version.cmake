# Apple's standard version keys contain integers. Keep the full beta identity
# in OBSStudioRustVersion; encode the daily build date as YYYY.M.D. A release
# without a beta date (the stable obs-studio-rust-v1.0.0) uses its UTC build
# date in the same form, so CFBundleVersion stays monotonic and Sparkle offers
# it over every earlier certified beta (OBS_BUILD_NUMBER would sort below them).
include_guard(GLOBAL)

function(obs_bundle_versions short_version build_version)
  set(short "${OBS_VERSION_CANONICAL}")
  set(build "${OBS_BUILD_NUMBER}")
  if(OBS_RUST_RELEASE_VERSION)
    string(REGEX REPLACE "^([0-9]+\\.[0-9]+\\.[0-9]+).*" "\\1" short "${OBS_RUST_RELEASE_VERSION}")
    if(OBS_RUST_RELEASE_VERSION MATCHES "-beta\\.([0-9][0-9][0-9][0-9])([0-9][0-9])([0-9][0-9])$")
      set(year "${CMAKE_MATCH_1}")
      set(month "${CMAKE_MATCH_2}")
      set(day "${CMAKE_MATCH_3}")
      string(REGEX REPLACE "^0" "" month "${month}")
      string(REGEX REPLACE "^0" "" day "${day}")
      set(build "${year}.${month}.${day}")
    else()
      string(TIMESTAMP year "%Y" UTC)
      string(TIMESTAMP month "%m" UTC)
      string(TIMESTAMP day "%d" UTC)
      string(REGEX REPLACE "^0" "" month "${month}")
      string(REGEX REPLACE "^0" "" day "${day}")
      set(build "${year}.${month}.${day}")
    endif()
  endif()
  set(${short_version} "${short}" PARENT_SCOPE)
  set(${build_version} "${build}" PARENT_SCOPE)
endfunction()
