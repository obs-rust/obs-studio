#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* Private adapters to the live mutex and real owner layouts. Rust checks NULL
 * before calling any owner accessor and acquires weak owners under the lock. */
bool obs_hotkey_register_lock(void);
bool obs_hotkey_register_lock(void)
{
	return obs_hotkey_seam_lock();
}

void obs_hotkey_register_unlock(void);
void obs_hotkey_register_unlock(void)
{
	obs_hotkey_seam_unlock();
}

bool obs_hotkey_register_source_private(obs_source_t *source);
bool obs_hotkey_register_source_private(obs_source_t *source)
{
	return source->context.private;
}

void obs_hotkey_register_encoder_fields(obs_encoder_t *owner, void **weak, struct obs_context_data **context);
void obs_hotkey_register_encoder_fields(obs_encoder_t *owner, void **weak, struct obs_context_data **context)
{
	*weak = obs_encoder_get_weak_encoder(owner);
	*context = &owner->context;
}

void obs_hotkey_register_output_fields(obs_output_t *owner, void **weak, struct obs_context_data **context);
void obs_hotkey_register_output_fields(obs_output_t *owner, void **weak, struct obs_context_data **context)
{
	*weak = obs_output_get_weak_output(owner);
	*context = &owner->context;
}

void obs_hotkey_register_service_fields(obs_service_t *owner, void **weak, struct obs_context_data **context);
void obs_hotkey_register_service_fields(obs_service_t *owner, void **weak, struct obs_context_data **context)
{
	*weak = obs_service_get_weak_service(owner);
	*context = &owner->context;
}

void obs_hotkey_register_source_fields(obs_source_t *owner, void **weak, struct obs_context_data **context);
void obs_hotkey_register_source_fields(obs_source_t *owner, void **weak, struct obs_context_data **context)
{
	*weak = obs_source_get_weak_source(owner);
	*context = &owner->context;
}
