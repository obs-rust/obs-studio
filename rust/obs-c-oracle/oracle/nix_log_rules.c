/* Test-only oracle: the original libobs/obs-nix-log-rules.c.h, unmodified,
 * with its five functions renamed to oracle_* so they can link next to the
 * Rust shims. It is plain C (strncmp(), strcspn() and strcmp()), so it builds
 * on every host. */
#include <stdbool.h>
#include <stddef.h>

#define obs_nix_system_info_os_release_field oracle_obs_nix_system_info_os_release_field
#define obs_nix_system_info_os_release_value oracle_obs_nix_system_info_os_release_value
#define obs_nix_system_info_version_fallback oracle_obs_nix_system_info_version_fallback
#define obs_nix_system_info_extension_name oracle_obs_nix_system_info_extension_name
#define obs_nix_system_info_desktop_form oracle_obs_nix_system_info_desktop_form

#include "obs-nix-log-rules.c.h"

#undef obs_nix_system_info_desktop_form
#undef obs_nix_system_info_extension_name
#undef obs_nix_system_info_version_fallback
#undef obs_nix_system_info_os_release_value
#undef obs_nix_system_info_os_release_field
