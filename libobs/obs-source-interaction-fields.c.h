#include "libobs/build/libobs.pch.h"

struct obs_source_interaction_fields obs_source_interaction_fields(const obs_source_t *source)
{
	return (struct obs_source_interaction_fields){
		source->info.output_flags, source->context.data, source->info.mouse_click, source->info.mouse_move,
		source->info.mouse_wheel,  source->info.focus,   source->info.key_click,
	};
}
