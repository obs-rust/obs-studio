#include "libobs/build/libobs.pch.h"

/* Private calls into the unchanged registry engine in obs-hotkey.c.h. */
extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);
extern obs_hotkey_id obs_hotkey_register_call(obs_hotkey_registerer_t type, void *registerer,
					      struct obs_context_data *context, const char *name,
					      const char *description, obs_hotkey_func func, void *data);

obs_hotkey_id obs_hotkey_register_frontend(const char *name, const char *description, obs_hotkey_func func, void *data)
{
	if (!obs_hotkey_seam_lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id =
		obs_hotkey_register_call(OBS_HOTKEY_REGISTERER_FRONTEND, NULL, NULL, name, description, func, data);

	obs_hotkey_seam_unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_encoder(obs_encoder_t *encoder, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!encoder || !obs_hotkey_seam_lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_call(OBS_HOTKEY_REGISTERER_ENCODER,
						    obs_encoder_get_weak_encoder(encoder), &encoder->context, name,
						    description, func, data);

	obs_hotkey_seam_unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_output(obs_output_t *output, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!output || !obs_hotkey_seam_lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_call(OBS_HOTKEY_REGISTERER_OUTPUT, obs_output_get_weak_output(output),
						    &output->context, name, description, func, data);

	obs_hotkey_seam_unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_service(obs_service_t *service, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!service || !obs_hotkey_seam_lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_call(OBS_HOTKEY_REGISTERER_SERVICE,
						    obs_service_get_weak_service(service), &service->context, name,
						    description, func, data);

	obs_hotkey_seam_unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_source(obs_source_t *source, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!source || source->context.private || !obs_hotkey_seam_lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_call(OBS_HOTKEY_REGISTERER_SOURCE, obs_source_get_weak_source(source),
						    &source->context, name, description, func, data);

	obs_hotkey_seam_unlock();
	return id;
}
