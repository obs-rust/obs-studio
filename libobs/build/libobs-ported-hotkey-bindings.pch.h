/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-hotkey-bindings. obs-hotkey-bindings.c needs only the
 * declarations of its three functions and obs_key_combination_t
 * (obs-internal.h, which brings obs.h with the obs_data and key name
 * functions and the INTERACT_* bits), so the C oracle and the C tests can
 * compile it on their own. The implementation file includes this header
 * and nothing else. */

#pragma once

#include "obs-internal.h"
