#include "libobs/build/libobs.pch.h"

struct obs_object_query_fields obs_object_query_fields(const void *obj)
{
	const struct obs_context_data *context = obj;
	return (struct obs_object_query_fields){&context->type, &context->private};
}
