#pragma once

#include "plugins/obs-vst3/VST3Scanner.h"

#include <obs-module.h>
#include <util/platform.h>

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#ifdef _WIN32
#include <windows.h>
#endif
