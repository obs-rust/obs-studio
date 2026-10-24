/* Test-only oracle: the original libobs/obs-source-async-pacing.c.h,
 * unmodified, with its function renamed to oracle_* so it can link next to
 * the Rust shim. */
#include <stddef.h>

#define obs_source_async_pacing oracle_obs_source_async_pacing

#include "obs-source-async-pacing.c.h"

#undef obs_source_async_pacing

/* The constants of the header, in the order of
 * rust/obs-c-oracle/src/source_async_pacing.rs. */
enum {
	ORACLE_SOURCE_ASYNC_PACING_CONSTANT_COUNT = 2,
};

unsigned long long oracle_source_async_pacing_constant(size_t which);

unsigned long long oracle_source_async_pacing_constant(size_t which)
{
	const unsigned long long values[ORACLE_SOURCE_ASYNC_PACING_CONSTANT_COUNT] = {
		OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR,
		OBS_SOURCE_ASYNC_PACING_FRAME_TOLERANCE,
	};
	return which < ORACLE_SOURCE_ASYNC_PACING_CONSTANT_COUNT ? values[which] : 0;
}

/* The layout of struct obs_source_async_pacing_result, for the Rust mirror:
 * its size, then the offsets of drop_front, last_frame_ts and take_first. */
size_t oracle_source_async_pacing_result_layout(size_t which);

size_t oracle_source_async_pacing_result_layout(size_t which)
{
	const size_t values[4] = {
		sizeof(struct obs_source_async_pacing_result),
		offsetof(struct obs_source_async_pacing_result, drop_front),
		offsetof(struct obs_source_async_pacing_result, last_frame_ts),
		offsetof(struct obs_source_async_pacing_result, take_first),
	};
	return which < 4 ? values[which] : 0;
}
