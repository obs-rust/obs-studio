/* Test-only oracle: the original libobs/obs-source-transition-sample-time.c.h,
 * unmodified, built twice so it can link next to the Rust shims:
 * - oracle_obs_source_transition_sample_time() asks the C clock,
 *   oracle_obs_source_transition_clock_time() (oracle/
 *   source_transition_clock.c), as libobs-c-reference composes them;
 * - oracle_obs_source_transition_sample_time_rust_clock() asks
 *   obs_source_transition_clock_time(), which in the test executables is
 *   the Rust shim, as the hybrid libobs's clock. Its results differ from
 *   the shim's only if the moved lines do: the clock's long double divide,
 *   whose width differs by platform, is the same on both sides. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_transition_sample_time oracle_obs_source_transition_sample_time
#define obs_source_transition_clock_time oracle_obs_source_transition_clock_time

#include "obs-source-transition-sample-time.c.h"

#undef obs_source_transition_clock_time
#undef obs_source_transition_sample_time

/* The precompiled header is included once, so these are declared here for
 * the second build. */
float obs_source_transition_clock_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts);
float oracle_obs_source_transition_sample_time_rust_clock(int mode, float manual_val, uint64_t start_time,
							  uint64_t duration, uint64_t ts, size_t sample_rate,
							  size_t sample);

#define obs_source_transition_sample_time oracle_obs_source_transition_sample_time_rust_clock

#include "obs-source-transition-sample-time.c.h"

#undef obs_source_transition_sample_time

/* Whether util_mul_div64() is MSVC's _udiv128(), which faults on a quotient
 * past 64 bits, in this build: util/util_uint64.h's condition. clang-cl's
 * __int128 division is cut to 64 bits instead, and the split formula
 * elsewhere wraps. */
size_t oracle_source_transition_sample_time_mul_div_faults(void);

size_t oracle_source_transition_sample_time_mul_div_faults(void)
{
#if defined(_MSC_VER) && defined(_M_X64) && (_MSC_VER >= 1920) && !defined(__clang__)
	return 1;
#else
	return 0;
#endif
}
