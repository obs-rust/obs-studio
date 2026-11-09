/* Link the real field adapters for Rust-static oracle consumers (the bench).
 * Keep this a separate archive member: category tests supply their own registry
 * accessors, while the benchmark uses the original-C enumeration fixture. */
#include "libobs/build/libobs.pch.h"

struct obs_core *oracle_hotkey_enum_core(void);
#define obs oracle_hotkey_enum_core()
#include "hotkey-enum-fields.inc"
#undef obs
