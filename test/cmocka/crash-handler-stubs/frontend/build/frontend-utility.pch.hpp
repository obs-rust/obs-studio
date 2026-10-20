/* Test stand-in for frontend/build/frontend-utility.pch.hpp, for
 * test_crash_log_upload: only the includes utility/CrashHandler.cpp takes
 * from it. This directory comes first on the test's include path. */

#pragma once

#include "frontend/utility/CrashHandler.hpp"
#include "frontend/utility/crash-sentinel-dir.hpp"
#include <OBSApp.hpp>

#include <nlohmann/json.hpp>

#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string_view>
