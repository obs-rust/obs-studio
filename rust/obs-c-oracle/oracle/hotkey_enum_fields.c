/* Real adapter definitions for standalone Rust-static consumers such as the
 * benchmark. Category tests supply their own dispatchers, so keep these in a
 * separate archive member. */
#include "libobs/build/libobs.pch.h"

struct obs_core *oracle_hotkey_enum_core(void);
#define obs oracle_hotkey_enum_core()
#include "hotkey-enum-fields.inc"
#undef obs
