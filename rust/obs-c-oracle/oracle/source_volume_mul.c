/* Test-only oracle: the original libobs/obs-source-volume-mul.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_source_volume_mul_block oracle_obs_source_volume_mul_block
#define obs_source_volume_mul_data oracle_obs_source_volume_mul_data

#include "obs-source-volume-mul.c.h"

#undef obs_source_volume_mul_data
#undef obs_source_volume_mul_block
