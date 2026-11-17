#include "libobs/build/libobs.pch.h"

void *obs_encoder_create_rerouted(obs_encoder_t *encoder, const char *reroute_id)
{
	if (!obs_ptr_valid(encoder, "obs_encoder_reroute"))
		return NULL;
	if (!obs_ptr_valid(reroute_id, "obs_encoder_reroute"))
		return NULL;
	if (!obs_encoder_reroute_allowed())
		return NULL;

	const struct obs_encoder_info *ei = find_encoder(reroute_id);
	if (ei) {
		if (ei->type != encoder->orig_info.type || astrcmpi(ei->codec, encoder->orig_info.codec) != 0) {
			return NULL;
		}
		encoder->info = *ei;
		return encoder->info.create(encoder->context.settings, encoder);
	}

	return NULL;
}
