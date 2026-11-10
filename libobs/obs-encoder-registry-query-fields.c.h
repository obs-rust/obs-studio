#include "libobs/build/libobs.pch.h"

struct obs_encoder_registry_query_fields obs_encoder_registry_query_fields(const struct obs_encoder_info *info)
{
	return (struct obs_encoder_registry_query_fields){&info->type, &info->caps};
}
