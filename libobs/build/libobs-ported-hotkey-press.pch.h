/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-hotkey-press. obs-hotkey-press.c needs only
 * obs_key_combination_t and the key codes (obs-hotkey.h), the INTERACT_*
 * bits (obs-interaction.h) and the declarations of its entry points and of
 * the binding accessor, key state and press and release wrappers it calls
 * (obs-internal.h, which brings obs.h), so the C oracle and the C tests can
 * compile it on their own. The implementation file includes this header
 * and nothing else. */

#pragma once

#include "obs-internal.h"
