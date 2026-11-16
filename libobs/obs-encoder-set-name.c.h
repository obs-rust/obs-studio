#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_encoder_set_name(obs_encoder_t *encoder, const char *name)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_name"))
		return;

	if (name && *name && strcmp(name, encoder->context.name) != 0)
		obs_context_data_setname(&encoder->context, name);
}
