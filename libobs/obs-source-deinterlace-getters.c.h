#include "libobs/build/libobs-ported-service-getters.pch.h"

enum obs_deinterlace_mode obs_source_get_deinterlace_mode(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_set_deinterlace_mode") ? source->deinterlace_mode
									   : OBS_DEINTERLACE_MODE_DISABLE;
}

enum obs_deinterlace_field_order obs_source_get_deinterlace_field_order(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_set_deinterlace_field_order"))
		return OBS_DEINTERLACE_FIELD_ORDER_TOP;

	return source->deinterlace_top_first ? OBS_DEINTERLACE_FIELD_ORDER_TOP : OBS_DEINTERLACE_FIELD_ORDER_BOTTOM;
}
