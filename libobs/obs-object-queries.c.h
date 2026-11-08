#include "libobs/build/libobs.pch.h"

enum obs_obj_type obs_obj_get_type(void *obj)
{
	struct obs_context_data *context = obj;
	return context ? context->type : OBS_OBJ_TYPE_INVALID;
}

bool obs_obj_is_private(void *obj)
{
	struct obs_context_data *context = obj;
	if (!context)
		return false;

	return context->private;
}
