/* Test-only oracle: the original libobs/obs-nix-system-info.c.h, unmodified,
 * with its eleven functions renamed to oracle_* so they can link next to the
 * Rust shims. It is plain C (strncmp(), strchr(), strrchr(), strcspn(),
 * atoi() and integer arithmetic), so it builds on every host. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_system_info_is_name oracle_obs_nix_system_info_is_name
#define obs_nix_system_info_is_physical_id oracle_obs_nix_system_info_is_physical_id
#define obs_nix_system_info_is_speed oracle_obs_nix_system_info_is_speed
#define obs_nix_system_info_value oracle_obs_nix_system_info_value
#define obs_nix_system_info_physical_id oracle_obs_nix_system_info_physical_id
#define obs_nix_system_info_prints oracle_obs_nix_system_info_prints
#define obs_nix_system_info_is_dmesg_cpu oracle_obs_nix_system_info_is_dmesg_cpu
#define obs_nix_system_info_dmesg_speed oracle_obs_nix_system_info_dmesg_speed
#define obs_nix_system_info_total_mb oracle_obs_nix_system_info_total_mb
#define obs_nix_system_info_free_mb oracle_obs_nix_system_info_free_mb
#define obs_nix_system_info_physmem_mb oracle_obs_nix_system_info_physmem_mb

#include "obs-nix-system-info.c.h"

#undef obs_nix_system_info_physmem_mb
#undef obs_nix_system_info_free_mb
#undef obs_nix_system_info_total_mb
#undef obs_nix_system_info_dmesg_speed
#undef obs_nix_system_info_is_dmesg_cpu
#undef obs_nix_system_info_prints
#undef obs_nix_system_info_physical_id
#undef obs_nix_system_info_value
#undef obs_nix_system_info_is_speed
#undef obs_nix_system_info_is_physical_id
#undef obs_nix_system_info_is_name
