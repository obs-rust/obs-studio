/* Test-only oracle: the original libobs/obs-output-protocols.c.h,
 * unmodified, with obs_output_protocol_list_count renamed so it can link
 * next to the Rust shim. */
#include <stddef.h>

#define obs_output_protocol_list_count oracle_obs_output_protocol_list_count

#include "obs-output-protocols.c.h"
