/* Verbatim original getter and discovered adapters over the real private header. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#define obs core_frame_interval_oracle_core
#define obs_get_frame_interval_ns oracle_obs_get_frame_interval_ns
#include "obs-core-frame-interval.c.h"
#undef obs_get_frame_interval_ns
#include "core_frame_interval_fields.inc"
#undef obs

static struct obs_core frame_interval_state;
static struct obs_core frame_interval_before;
struct obs_core *core_frame_interval_oracle_core = &frame_interval_state;

void oracle_core_frame_interval_set(uint64_t value);
void oracle_core_frame_interval_set(uint64_t value)
{
	frame_interval_state.video.video_frame_interval_ns = value;
	memcpy(&frame_interval_before, &frame_interval_state, sizeof(frame_interval_state));
}

bool oracle_core_frame_interval_unchanged(void);
bool oracle_core_frame_interval_unchanged(void)
{
	return core_frame_interval_oracle_core == &frame_interval_state &&
	       memcmp(&frame_interval_before, &frame_interval_state, sizeof(frame_interval_state)) == 0;
}

void oracle_core_frame_interval_layout(size_t *layout);
void oracle_core_frame_interval_layout(size_t *layout)
{
	layout[0] = sizeof(frame_interval_state.video.video_frame_interval_ns);
	layout[1] = _Alignof(uint64_t);
}
