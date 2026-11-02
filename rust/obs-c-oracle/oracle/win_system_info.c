/* Test-only oracle: the original libobs/obs-windows-system-info.c.h,
 * unmodified, with its ten functions renamed to oracle_* so they can link
 * next to the Rust shims. It takes no Windows type (integer compares, a shift,
 * a divide by a constant and two label tables), so it builds on every host. */
#include <stdbool.h>
#include <stdint.h>

#define obs_windows_system_info_version oracle_obs_windows_system_info_version
#define obs_windows_system_info_mb oracle_obs_windows_system_info_mb
#define obs_windows_system_info_gaming_logged oracle_obs_windows_system_info_gaming_logged
#define obs_windows_system_info_game_mode_retry oracle_obs_windows_system_info_game_mode_retry
#define obs_windows_system_info_gaming_found oracle_obs_windows_system_info_gaming_found
#define obs_windows_system_info_gaming_value_line oracle_obs_windows_system_info_gaming_value_line
#define obs_windows_system_info_gaming_fallback oracle_obs_windows_system_info_gaming_fallback
#define obs_windows_system_info_gaming_fallback_line oracle_obs_windows_system_info_gaming_fallback_line
#define obs_windows_system_info_product_state oracle_obs_windows_system_info_product_state
#define obs_windows_system_info_product_type oracle_obs_windows_system_info_product_type

#include "obs-windows-system-info.c.h"

#undef obs_windows_system_info_product_type
#undef obs_windows_system_info_product_state
#undef obs_windows_system_info_gaming_fallback_line
#undef obs_windows_system_info_gaming_fallback
#undef obs_windows_system_info_gaming_value_line
#undef obs_windows_system_info_gaming_found
#undef obs_windows_system_info_game_mode_retry
#undef obs_windows_system_info_gaming_logged
#undef obs_windows_system_info_mb
#undef obs_windows_system_info_version
