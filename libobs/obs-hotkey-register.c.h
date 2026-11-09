#include "libobs/build/libobs.pch.h"

/* Private one-call seams keep the C reference independent of the core unity group. */
#define lock obs_hotkey_register_lock
#define unlock obs_hotkey_register_unlock
#define obs_hotkey_register_internal obs_hotkey_register_insert

extern bool obs_hotkey_register_lock(void);
extern void obs_hotkey_register_unlock(void);
extern obs_hotkey_id obs_hotkey_register_insert(obs_hotkey_registerer_t type, void *registerer,
						struct obs_context_data *context, const char *name,
						const char *description, obs_hotkey_func func, void *data);

obs_hotkey_id obs_hotkey_register_frontend(const char *name, const char *description, obs_hotkey_func func, void *data)
{
	if (!lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id =
		obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_FRONTEND, NULL, NULL, name, description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_encoder(obs_encoder_t *encoder, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!encoder || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_ENCODER,
							obs_encoder_get_weak_encoder(encoder), &encoder->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_output(obs_output_t *output, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!output || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_OUTPUT,
							obs_output_get_weak_output(output), &output->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_service(obs_service_t *service, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!service || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_SERVICE,
							obs_service_get_weak_service(service), &service->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_source(obs_source_t *source, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!source || source->context.private || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_SOURCE,
							obs_source_get_weak_source(source), &source->context, name,
							description, func, data);

	unlock();
	return id;
}

#undef obs_hotkey_register_internal
#undef unlock
#undef lock
