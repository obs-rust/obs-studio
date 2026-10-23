/* Test-only oracle: libobs/obs-encoder-roi.c.h, unmodified, with its three
 * functions renamed so they can link next to the Rust shims, plus the
 * layout of struct obs_encoder_roi and OBS_ENCODER_CAP_ROI as the C sees
 * them. */
#include <stddef.h>

#define obs_encoder_roi_admit oracle_obs_encoder_roi_admit
#define obs_encoder_roi_scale_factors oracle_obs_encoder_roi_scale_factors
#define obs_encoder_roi_scale oracle_obs_encoder_roi_scale

#include "obs-encoder-roi.c.h"

#undef obs_encoder_roi_scale
#undef obs_encoder_roi_scale_factors
#undef obs_encoder_roi_admit

/* In the order of rust/obs-c-oracle/src/encoder_roi.rs. */
enum {
	ORACLE_ENCODER_ROI_LAYOUT_COUNT = 7,
};

uint32_t oracle_encoder_roi_cap(void);
size_t oracle_encoder_roi_layout(size_t which);

uint32_t oracle_encoder_roi_cap(void)
{
	return OBS_ENCODER_CAP_ROI;
}

size_t oracle_encoder_roi_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_ROI_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_roi),
		_Alignof(struct obs_encoder_roi),
		offsetof(struct obs_encoder_roi, top),
		offsetof(struct obs_encoder_roi, bottom),
		offsetof(struct obs_encoder_roi, left),
		offsetof(struct obs_encoder_roi, right),
		offsetof(struct obs_encoder_roi, priority),
	};
	return which < ORACLE_ENCODER_ROI_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
