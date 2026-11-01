/* Test-only oracle: the original
 * libobs/audio-monitoring/osx/coreaudio-device-rules.c.h, unmodified, with its
 * five functions renamed to oracle_* so they can link next to the Rust shims.
 * Its virtual device check is the oracle's C dstr (oracle/dstr.c: astrstri
 * with C's tolower()), and its compares strcmp(). */
#include <stddef.h>

#define astrstri oracle_astrstri

#define coreaudio_device_listed oracle_coreaudio_device_listed
#define coreaudio_device_count oracle_coreaudio_device_count
#define coreaudio_device_is_default oracle_coreaudio_device_is_default
#define coreaudio_device_names_match oracle_coreaudio_device_names_match
#define coreaudio_device_is_input oracle_coreaudio_device_is_input

#include "audio-monitoring/osx/coreaudio-device-rules.c.h"
