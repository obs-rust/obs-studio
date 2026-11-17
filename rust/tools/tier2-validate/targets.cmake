# One native build invocation, with the same roots as the validators used to
# pass individually. In particular, MSBuild otherwise traverses shared core
# projects again for each --target argument (Refs #4996).
add_custom_target(tier2-native)
add_dependencies(tier2-native libobs libobs-c-reference cmocka-tests libobs-opengl)
if(OS_WINDOWS)
  add_dependencies(tier2-native libobs-d3d11)
elseif(OS_MACOS)
  add_dependencies(tier2-native libobs-metal mac-capture mac-avcapture mac-avcapture-legacy)
else()
  message(FATAL_ERROR "tier2-native requires Windows or macOS")
endif()
