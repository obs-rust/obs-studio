/* Test-only oracle: the original libobs/obs-output-reconnect.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. The parity test never passes the inputs where C is
 * undefined (a delay setting whose * 1000 overflows int, a later product
 * above UINT32_MAX or below 0, or not a number). */
#include <stdbool.h>
#include <stdint.h>

#define obs_output_reconnect_first_delay oracle_obs_output_reconnect_first_delay
#define obs_output_reconnect_give_up oracle_obs_output_reconnect_give_up
#define obs_output_reconnect_grows oracle_obs_output_reconnect_grows
#define obs_output_reconnect_next_delay oracle_obs_output_reconnect_next_delay
#define obs_output_reconnect_active oracle_obs_output_reconnect_active
#define obs_output_reconnect_can oracle_obs_output_reconnect_can

#include "obs-output-reconnect.c.h"

#undef obs_output_reconnect_can
#undef obs_output_reconnect_active
#undef obs_output_reconnect_next_delay
#undef obs_output_reconnect_grows
#undef obs_output_reconnect_give_up
#undef obs_output_reconnect_first_delay
