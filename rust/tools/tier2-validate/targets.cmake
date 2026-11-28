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

# What only a native macOS build can provide (#5848): both libobs cores and the
# graphics/capture reference modules the Cargo-native contract compares against.
# The cmocka executables come prebuilt from the Linux cross-build bundle.
add_custom_target(tier2-residue)
# The ASan variants, cmocka-asan-tests, cannot be cross-built: soldr's LLVM 21 ASan
# runtime deadlocks at init on macOS 26. test_bitstream is the loader probe.
add_dependencies(
  tier2-residue
  libobs
  libobs-c-reference
  libobs-opengl
  cmocka-asan-tests
  test_bitstream
)
if(OS_MACOS)
  add_dependencies(tier2-residue libobs-metal mac-capture mac-avcapture mac-avcapture-legacy)
endif()
