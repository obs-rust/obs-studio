#include "libobs/build/libobs.pch.h"

/* Original implementation shared by the C reference and differential oracle. */
bool obs_encoder_add_roi(obs_encoder_t *encoder, const struct obs_encoder_roi *roi);
void obs_encoder_enum_roi(obs_encoder_t *encoder, void (*enum_proc)(void *, struct obs_encoder_roi *), void *param);

#include "rust/obs-c-oracle/legacy/obs-encoder-roi-list.inc"
