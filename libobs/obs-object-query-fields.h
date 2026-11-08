#pragma once

#include "obs.h"

struct obs_object_query_fields {
	const enum obs_obj_type *type;
	const bool *private;
};
extern struct obs_object_query_fields obs_object_query_fields(const void *obj);
