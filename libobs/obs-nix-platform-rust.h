/* Private state seam: the C setter preserves the C build's assertion policy. */
#pragma once

#include "libobs/obs-nix-platform.h"

#ifdef __cplusplus
extern "C" {
#endif

void obs_nix_platform_set(enum obs_nix_platform_type platform);

#ifdef __cplusplus
}
#endif
