#include "libobs/build/libobs.pch.h"

struct obs_encoder_reroute_fields obs_encoder_reroute_fields(obs_encoder_t *encoder,
							     const struct obs_encoder_info *info)
{
	return (struct obs_encoder_reroute_fields){&encoder->info,  info,
						   sizeof(*info),   (int)encoder->orig_info.type,
						   (int)info->type, encoder->orig_info.codec,
						   info->codec,     encoder->context.settings,
						   info->create};
}
