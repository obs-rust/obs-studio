#include "libobs/build/libobs.pch.h"

struct obs_source_deinterlace_fields obs_source_deinterlace_fields(const obs_source_t *source)
{
	return (struct obs_source_deinterlace_fields){source ? &source->deinterlace_mode : NULL,
						      source ? &source->deinterlace_top_first : NULL};
}
