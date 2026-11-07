# Defined in build/test-graph.json for both CMake and Cargo consumers.
# Preserve the unchanged compatibility test and its platform/header requirements.
# Linux retains the FreeType/SIMDe pkg-config coverage. Native Windows/macOS
# Tier 2 SDKs do not supply those optional packages; skip before looking them up.
