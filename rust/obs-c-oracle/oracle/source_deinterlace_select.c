/* Test-only oracle: the original libobs/obs-source-deinterlace-select.c.h,
 * unmodified, with its eight functions renamed to oracle_* so they can link
 * next to the Rust shims. The two pacing functions it calls are renamed too,
 * so it calls the C of oracle/source_deinterlace_pacing.c, not the shims. */
#include <stddef.h>

#define obs_source_deinterlace_pacing_wait_for_next oracle_obs_source_deinterlace_pacing_wait_for_next
#define obs_source_deinterlace_pacing_waiting_frame_ts oracle_obs_source_deinterlace_pacing_waiting_frame_ts
#define obs_source_deinterlace_select_first oracle_obs_source_deinterlace_select_first
#define obs_source_deinterlace_select_walk oracle_obs_source_deinterlace_select_walk
#define obs_source_deinterlace_select_unbuffered_pair oracle_obs_source_deinterlace_select_unbuffered_pair
#define obs_source_deinterlace_select_take_reads_flag oracle_obs_source_deinterlace_select_take_reads_flag
#define obs_source_deinterlace_select_take_pair oracle_obs_source_deinterlace_select_take_pair
#define obs_source_deinterlace_select_take_init_last oracle_obs_source_deinterlace_select_take_init_last
#define obs_source_deinterlace_select_take_offset oracle_obs_source_deinterlace_select_take_offset
#define obs_source_deinterlace_select_update oracle_obs_source_deinterlace_select_update

#include "obs-source-deinterlace-select.c.h"

#undef obs_source_deinterlace_select_update
#undef obs_source_deinterlace_select_take_offset
#undef obs_source_deinterlace_select_take_init_last
#undef obs_source_deinterlace_select_take_pair
#undef obs_source_deinterlace_select_take_reads_flag
#undef obs_source_deinterlace_select_unbuffered_pair
#undef obs_source_deinterlace_select_walk
#undef obs_source_deinterlace_select_first
#undef obs_source_deinterlace_pacing_waiting_frame_ts
#undef obs_source_deinterlace_pacing_wait_for_next

/* The header's constant and enum values, in the order of
 * rust/obs-c-oracle/src/source_deinterlace_select.rs. */
enum {
	ORACLE_SOURCE_DEINTERLACE_SELECT_CONSTANT_COUNT = 6,
};

unsigned long long oracle_source_deinterlace_select_constant(size_t which);

unsigned long long oracle_source_deinterlace_select_constant(size_t which)
{
	const unsigned long long values[ORACLE_SOURCE_DEINTERLACE_SELECT_CONSTANT_COUNT] = {
		OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR,      OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED,
		OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR, OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE,
		OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD,   OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP,
	};
	return which < ORACLE_SOURCE_DEINTERLACE_SELECT_CONSTANT_COUNT ? values[which] : 0;
}

/* The layouts of the three result structs and the sizes of the two enums,
 * for the Rust mirrors: first_result's size and the offsets of first and
 * flag_front; walk_result's size and the offsets of drop_front,
 * last_frame_ts, offset_reset, pairs, flag_front and ready; pair_result's
 * size and the offsets of prev_frame, set_frame_ts and frame_ts; the sizes
 * of the offset kind and the update step. */
size_t oracle_source_deinterlace_select_layout(size_t which);

size_t oracle_source_deinterlace_select_layout(size_t which)
{
	const size_t values[16] = {
		sizeof(struct obs_source_deinterlace_select_first_result),
		offsetof(struct obs_source_deinterlace_select_first_result, first),
		offsetof(struct obs_source_deinterlace_select_first_result, flag_front),
		sizeof(struct obs_source_deinterlace_select_walk_result),
		offsetof(struct obs_source_deinterlace_select_walk_result, drop_front),
		offsetof(struct obs_source_deinterlace_select_walk_result, last_frame_ts),
		offsetof(struct obs_source_deinterlace_select_walk_result, offset_reset),
		offsetof(struct obs_source_deinterlace_select_walk_result, pairs),
		offsetof(struct obs_source_deinterlace_select_walk_result, flag_front),
		offsetof(struct obs_source_deinterlace_select_walk_result, ready),
		sizeof(struct obs_source_deinterlace_select_pair_result),
		offsetof(struct obs_source_deinterlace_select_pair_result, prev_frame),
		offsetof(struct obs_source_deinterlace_select_pair_result, set_frame_ts),
		offsetof(struct obs_source_deinterlace_select_pair_result, frame_ts),
		sizeof(enum obs_source_deinterlace_select_offset_kind),
		sizeof(enum obs_source_deinterlace_select_update_step),
	};
	return which < 16 ? values[which] : 0;
}
