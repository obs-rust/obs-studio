/* Precompiled header (BUILD.md) for the plugins/obs-nvenc/obs-nvenc-test unity groups
 * obs-nvenc-test.core. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* obs-nvenc-test.cpp */
#include <string_view>
#include <unordered_map>
#include <vector>
#include <chrono>
#include <future>
#include <cstring>

/* obs-nvenc-test.cpp.hpp declares the NVML types it loads by hand. This
 * makes nv-codec-headers' dynlink_nvml.h (newer than n13.1.15.0) skip its
 * own, which clash; older headers ignore it. */
#define NVML_API_VERSION 1
#include <ffnvcodec/nvEncodeAPI.h>
#include <ffnvcodec/dynlink_loader.h>
