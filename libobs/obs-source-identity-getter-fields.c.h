#include "libobs/build/libobs.pch.h"

struct obs_source_identity_getter_fields obs_source_identity_getter_fields(const obs_source_t *source)
{
	return (struct obs_source_identity_getter_fields){&source->context.uuid, &source->info.unversioned_id};
}
