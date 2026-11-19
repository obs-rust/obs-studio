/* Only the recording fixture substitutes the atomic operation. */
#include "libobs/build/libobs.pch.h"
bool output_starting_recording_load_bool(const volatile bool *flag);
#undef os_atomic_load_bool
#define os_atomic_load_bool output_starting_recording_load_bool
#include "libobs/obs-output-starting-queries-fields.c.h"
#undef os_atomic_load_bool
