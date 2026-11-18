#include "libobs/build/libobs.pch.h"

const char *obs_obj_get_id(void *obj)
{
	struct obs_context_data *context = obj;
	if (!context)
		return NULL;

	switch (context->type) {
	case OBS_OBJ_TYPE_SOURCE:
		return ((obs_source_t *)obj)->info.id;
	case OBS_OBJ_TYPE_OUTPUT:
		return ((obs_output_t *)obj)->info.id;
	case OBS_OBJ_TYPE_ENCODER:
		return ((obs_encoder_t *)obj)->info.id;
	case OBS_OBJ_TYPE_SERVICE:
		return ((obs_service_t *)obj)->info.id;
	default:;
	}

	return NULL;
}
