/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.macos-coreaudio-device-rules: the CoreAudio monitoring device
 * rules, without an Apple header, so the C tests compile it on their own.
 * C. Its implementation file includes this header and nothing else. */

#pragma once

#include <string.h>

#include "audio-monitoring/osx/coreaudio-device-rules.h"
#include "util/dstr.h"
