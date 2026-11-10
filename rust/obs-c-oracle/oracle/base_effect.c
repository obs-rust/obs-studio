/* Original C and real-layout fixture: pointers are borrowed tokens, never freed. */
#include <string.h>
#define obs base_effect_oracle_core
#define obs_get_base_effect oracle_obs_get_base_effect
#include "obs-base-effect.c.h"
#undef obs_get_base_effect
#undef obs
#include "base_effect_fields.inc"

static struct obs_core base_effect_state;
static struct obs_core base_effect_before;
struct obs_core *base_effect_oracle_core;

void *oracle_base_effect_set(gs_effect_t *const *effects);
void *oracle_base_effect_set(gs_effect_t *const *effects)
{
	base_effect_state.video.default_effect = effects[0];
	base_effect_state.video.default_rect_effect = effects[1];
	base_effect_state.video.opaque_effect = effects[2];
	base_effect_state.video.solid_effect = effects[3];
	base_effect_state.video.bicubic_effect = effects[4];
	base_effect_state.video.lanczos_effect = effects[5];
	base_effect_state.video.bilinear_lowres_effect = effects[6];
	base_effect_state.video.premultiplied_alpha_effect = effects[7];
	base_effect_state.video.repeat_effect = effects[8];
	base_effect_state.video.area_effect = effects[9];
	memcpy(&base_effect_before, &base_effect_state, sizeof(base_effect_state));
	base_effect_oracle_core = &base_effect_state;
	return &base_effect_state;
}

void oracle_base_effect_clear(void);
void oracle_base_effect_clear(void)
{
	base_effect_oracle_core = NULL;
}

bool oracle_base_effect_unchanged(void);
bool oracle_base_effect_unchanged(void)
{
	return memcmp(&base_effect_before, &base_effect_state, sizeof(base_effect_state)) == 0;
}
