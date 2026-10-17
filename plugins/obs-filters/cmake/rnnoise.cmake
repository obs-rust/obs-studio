option(ENABLE_RNNOISE "Enable building with RNNoise noise supression filter" ON)

if(ENABLE_RNNOISE)
  if(OS_WINDOWS OR OS_MACOS)
    find_package(Librnnoise REQUIRED)
  else()
    find_package(Librnnoise)

    if(NOT TARGET Librnnoise::Librnnoise)
      message(WARNING "No RNNoise library found. Using internal RNNoise version instead.")
      add_library(obs-rnnoise OBJECT)

      # Canonical native source list shared with the Cargo build.
      set(_obs_rnnoise_manifest "${CMAKE_CURRENT_SOURCE_DIR}/build/rnnoise-sources.json")
      set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_obs_rnnoise_manifest}")
      file(READ "${_obs_rnnoise_manifest}" _obs_rnnoise_json)
      string(JSON _obs_rnnoise_count LENGTH "${_obs_rnnoise_json}")
      math(EXPR _obs_rnnoise_last "${_obs_rnnoise_count} - 1")
      set(_obs_rnnoise_sources "")
      foreach(_index RANGE ${_obs_rnnoise_last})
        string(JSON _source GET "${_obs_rnnoise_json}" ${_index})
        list(APPEND _obs_rnnoise_sources "${_source}")
      endforeach()

      target_sources(
        obs-rnnoise
        PRIVATE
          ${_obs_rnnoise_sources}
          rnnoise/src/_kiss_fft_guts.h
          rnnoise/src/arch.h
          rnnoise/src/celt_lpc.h
          rnnoise/src/common.h
          rnnoise/src/kiss_fft.h
          rnnoise/src/opus_types.h
          rnnoise/src/pitch.h
          rnnoise/src/rnn.h
          rnnoise/src/rnn_data.h
          rnnoise/src/tansig_table.h
        PUBLIC rnnoise/include/rnnoise.h
      )

      add_library(Librnnoise::Librnnoise ALIAS obs-rnnoise)

      target_include_directories(obs-rnnoise PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/rnnoise/include")

      target_compile_definitions(obs-rnnoise PUBLIC COMPILE_OPUS)

      target_compile_options(
        obs-rnnoise
        PRIVATE
          -Wno-newline-eof
          -Wno-error=null-dereference
          -Wno-double-promotion
          -Wno-cast-qual
          $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-shadow>
      )

      set_target_properties(obs-rnnoise PROPERTIES FOLDER plugins/obs-filters/rnnoise POSITION_INDEPENDENT_CODE TRUE)
    endif()
  endif()

  target_sources(obs-filters PRIVATE build/obs-filters.noise-suppress+.h.c)
  # rnnoise.h uses ssize_t, which MSVC lacks. Only this file includes it, so
  # the define is scoped to it (was an #ifdef _MSC_VER in the source, #864).
  set_property(
    SOURCE build/obs-filters.noise-suppress+.h.c
    APPEND
    PROPERTY COMPILE_DEFINITIONS "$<$<C_COMPILER_ID:MSVC>:ssize_t=intptr_t>"
  )

  target_link_libraries(obs-filters PRIVATE Librnnoise::Librnnoise)

  target_enable_feature(obs-filters "RNNoise noise suppression" LIBRNNOISE_ENABLED HAS_NOISEREDUCTION)
else()
  target_disable_feature(obs-filters "RNNoise noise suppression")
endif()
