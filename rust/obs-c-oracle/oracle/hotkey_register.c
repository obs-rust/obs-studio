/* Execute the actual extracted wrappers with instrumented dependency calls.
 * Only the private owner layout is replaced. Tier 2 uses real libobs objects. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "obs.h"

struct obs_context_data {
	bool private;
	uintptr_t token;
};
struct owner {
	struct obs_context_data context;
};
struct obs_source {
	struct obs_context_data context;
};
struct obs_output {
	struct obs_context_data context;
};
struct obs_encoder {
	struct obs_context_data context;
};
struct obs_service {
	struct obs_context_data context;
};

extern void *obs_hotkey_register_weak(obs_hotkey_registerer_t type, void *owner);
extern obs_hotkey_id obs_hotkey_register_insert(obs_hotkey_registerer_t type, void *owner,
						struct obs_context_data *context, const char *name,
						const char *description, obs_hotkey_func func, void *data);

#define obs_source_get_weak_source(o) obs_hotkey_register_weak(OBS_HOTKEY_REGISTERER_SOURCE, (void *)(o))
#define obs_output_get_weak_output(o) obs_hotkey_register_weak(OBS_HOTKEY_REGISTERER_OUTPUT, (void *)(o))
#define obs_encoder_get_weak_encoder(o) obs_hotkey_register_weak(OBS_HOTKEY_REGISTERER_ENCODER, (void *)(o))
#define obs_service_get_weak_service(o) obs_hotkey_register_weak(OBS_HOTKEY_REGISTERER_SERVICE, (void *)(o))
#define obs_hotkey_register_frontend oracle_obs_hotkey_register_frontend
#define obs_hotkey_register_source oracle_obs_hotkey_register_source
#define obs_hotkey_register_output oracle_obs_hotkey_register_output
#define obs_hotkey_register_encoder oracle_obs_hotkey_register_encoder
#define obs_hotkey_register_service oracle_obs_hotkey_register_service

obs_hotkey_id oracle_obs_hotkey_register_frontend(const char *, const char *, obs_hotkey_func, void *);
obs_hotkey_id oracle_obs_hotkey_register_source(obs_source_t *, const char *, const char *, obs_hotkey_func, void *);
obs_hotkey_id oracle_obs_hotkey_register_output(obs_output_t *, const char *, const char *, obs_hotkey_func, void *);
obs_hotkey_id oracle_obs_hotkey_register_encoder(obs_encoder_t *, const char *, const char *, obs_hotkey_func, void *);
obs_hotkey_id oracle_obs_hotkey_register_service(obs_service_t *, const char *, const char *, obs_hotkey_func, void *);
#include "obs-hotkey-register.inc"

/* Rust supplies genuine C-layout fake owners without mirroring any OBS layout. */
void *oracle_hotkey_register_owner_new(bool private_owner, uintptr_t token);
void oracle_hotkey_register_owner_free(void *owner);
uintptr_t oracle_hotkey_register_owner_token(const void *owner);
bool oracle_hotkey_register_owner_private(const void *owner);
void *oracle_hotkey_register_owner_context(void *owner);
void oracle_hotkey_register_constants(size_t *out);

#include <stdlib.h>
void *oracle_hotkey_register_owner_new(bool private_owner, uintptr_t token)
{
	struct owner *owner = malloc(sizeof(*owner));
	if (owner) {
		owner->context.private = private_owner;
		owner->context.token = token;
	}
	return owner;
}
void oracle_hotkey_register_owner_free(void *owner)
{
	free(owner);
}
uintptr_t oracle_hotkey_register_owner_token(const void *owner)
{
	return ((const struct owner *)owner)->context.token;
}
bool oracle_hotkey_register_owner_private(const void *owner)
{
	return ((const struct owner *)owner)->context.private;
}
void *oracle_hotkey_register_owner_context(void *owner)
{
	return &((struct owner *)owner)->context;
}
void oracle_hotkey_register_constants(size_t *out)
{
	out[0] = OBS_INVALID_HOTKEY_ID;
	out[1] = OBS_HOTKEY_REGISTERER_FRONTEND;
	out[2] = OBS_HOTKEY_REGISTERER_SOURCE;
	out[3] = OBS_HOTKEY_REGISTERER_OUTPUT;
	out[4] = OBS_HOTKEY_REGISTERER_ENCODER;
	out[5] = OBS_HOTKEY_REGISTERER_SERVICE;
}
