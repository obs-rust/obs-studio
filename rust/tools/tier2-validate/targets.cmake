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

# The nightly ASan job (#5848): only the ASan variants cannot be cross-built, because
# soldr's LLVM 21 ASan runtime deadlocks at init on macOS 26 (forge-ci/soldr#4291).
# Everything else comes prebuilt from the Linux bundle. test_bitstream is the loader probe.
add_custom_target(tier2-residue)
add_dependencies(tier2-residue libobs libobs-c-reference cmocka-asan-tests test_bitstream)
