/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-hotkey-translations. obs-hotkey-translations.c needs only
 * struct obs_hotkeys_translations and the key codes (obs-hotkey.h), the dstr
 * functions and the declaration of obs_hotkeys_store_key_translation()
 * (obs-internal.h, which brings obs.h and util/dstr.h), so the C oracle and
 * the C tests can compile it on their own. The implementation file includes
 * this header and nothing else. */

#pragma once

#include "obs-internal.h"
