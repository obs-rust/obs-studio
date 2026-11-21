/* Compile the original platform state without its unrelated host PCH. */
#include <assert.h>
#include <stddef.h>
#define obs_set_nix_platform oracle_obs_set_nix_platform
#define obs_nix_platform_set oracle_obs_set_nix_platform
#define obs_get_nix_platform oracle_obs_get_nix_platform
#define obs_set_nix_platform_display oracle_obs_set_nix_platform_display
#define obs_get_nix_platform_display oracle_obs_get_nix_platform_display
#include "libobs/obs-nix-platform.h"
#include "obs-nix-platform.inc"
