#include "libobs/build/libobs.pch.h"

void *const *obs_object_validity_field(const void *obj)
{
	const struct obs_context_data *context = obj;
	return &context->data;
}
