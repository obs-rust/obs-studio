cmake_minimum_required(VERSION 3.28...3.30)

add_executable(
  obs-vst3-scanner
  build/obs-vst3-scanner+.hpp.cpp
  "$<$<PLATFORM_ID:Windows>:build/obs-vst3-scanner.windows+.hpp.cpp>"
  "$<$<PLATFORM_ID:Darwin,Linux>:build/obs-vst3-scanner.posix+.hpp.cpp>"
)
target_link_libraries(obs-vst3-scanner PRIVATE obs-vst3-discovery)
if(OS_WINDOWS)
  obs_msvc_exempt_c4201(obs-vst3-scanner)
endif()
set_target_properties_obs(obs-vst3-scanner PROPERTIES FOLDER plugins/obs-vst)
