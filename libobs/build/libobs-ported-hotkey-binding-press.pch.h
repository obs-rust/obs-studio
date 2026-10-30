/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-hotkey-binding-press. obs-hotkey-binding-press.c needs only
 * the hotkey types (obs-hotkey.h) and the declarations of the binding's count
 * accessor and of the dispatch it calls (obs-internal.h, which brings obs.h),
 * so the C oracle and the C tests can compile it on their own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "obs-internal.h"
