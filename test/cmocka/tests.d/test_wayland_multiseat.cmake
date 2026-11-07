# Headless Wayland compatibility fixture; preserve the original Linux/FreeBSD
# and ENABLE_WAYLAND guards. Missing optional SDKs select no test.
include("${CMAKE_CURRENT_LIST_DIR}/../wayland-test-sdk.cmake")
# Target and test are declared in ../build/test-graph.json.
