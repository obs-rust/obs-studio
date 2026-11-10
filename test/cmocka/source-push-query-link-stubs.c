/* Real private fields for standalone Rust-static consumers; legacy filename. */
#include "obs-internal.h"

struct obs_source_push_query_fields obs_source_push_query_fields(obs_source_t *source)
{
	return (struct obs_source_push_query_fields){&source->audio_mutex, &source->push_to_mute_enabled,
						     &source->push_to_talk_enabled, &source->push_to_mute_delay,
						     &source->push_to_talk_delay};
}
