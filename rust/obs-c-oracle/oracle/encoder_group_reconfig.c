/* Test-only oracle: the original libobs/obs-encoder-group-reconfig.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims, plus the layout of its structs as the C sees them.
 * The parity test never passes the inputs where C is undefined (an LCM
 * product above INT_MAX, a 0 or a value above INT_MAX in lcm(), a timestep
 * of 0, next_pts beyond int64_t). */
#include <stddef.h>
#include <stdint.h>

#define obs_encoder_group_reconfig_new_request oracle_obs_encoder_group_reconfig_new_request
#define obs_encoder_group_reconfig_update oracle_obs_encoder_group_reconfig_update
#define obs_encoder_group_reconfig_frame oracle_obs_encoder_group_reconfig_frame
#define obs_encoder_group_reconfig_lcm oracle_obs_encoder_group_reconfig_lcm
#define obs_encoder_group_reconfig_reset oracle_obs_encoder_group_reconfig_reset

#include "obs-encoder-group-reconfig.c.h"

#undef obs_encoder_group_reconfig_reset
#undef obs_encoder_group_reconfig_lcm
#undef obs_encoder_group_reconfig_frame
#undef obs_encoder_group_reconfig_update
#undef obs_encoder_group_reconfig_new_request

/* In the order of rust/obs-c-oracle/src/encoder_group_reconfig.rs. */
enum {
	ORACLE_ENCODER_GROUP_RECONFIG_LAYOUT_COUNT = 11,
};

size_t oracle_encoder_group_reconfig_layout(size_t which);

size_t oracle_encoder_group_reconfig_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_GROUP_RECONFIG_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_group_reconfig),
		_Alignof(struct obs_encoder_group_reconfig),
		offsetof(struct obs_encoder_group_reconfig, reconfigure_request),
		offsetof(struct obs_encoder_group_reconfig, next_pts),
		offsetof(struct obs_encoder_group_reconfig, encoders_updated_next_pts),
		offsetof(struct obs_encoder_group_reconfig, encoders_reconfigured),
		offsetof(struct obs_encoder_group_reconfig, reconfigure_again),
		sizeof(struct obs_encoder_reconfig),
		_Alignof(struct obs_encoder_reconfig),
		offsetof(struct obs_encoder_reconfig, last_reconfigure_request),
		offsetof(struct obs_encoder_reconfig, last_handled_reconfigure_request),
	};
	return which < ORACLE_ENCODER_GROUP_RECONFIG_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
