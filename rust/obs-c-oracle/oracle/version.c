/* Original version getters and the production adapter over an oracle-owned
 * OBS_VERSION, with the numbers of the real obs-config.h. */
#include <stddef.h>
#include <stdint.h>
#define OBS_VERSION version_oracle_string
#define obs_get_version oracle_obs_get_version
#define obs_get_version_string oracle_obs_get_version_string
#include "obs-version.c.h"
#undef obs_get_version
#undef obs_get_version_string
#include "version_fields.inc"
#undef OBS_VERSION

const char *version_oracle_string;

void oracle_version_set_string(const char *version);
void oracle_version_set_string(const char *version)
{
	version_oracle_string = version;
}

void oracle_version_parts(uint32_t *parts);
void oracle_version_parts(uint32_t *parts)
{
	parts[0] = LIBOBS_API_MAJOR_VER;
	parts[1] = LIBOBS_API_MINOR_VER;
	parts[2] = LIBOBS_API_PATCH_VER;
	parts[3] = LIBOBS_API_VER;
}

uint32_t oracle_make_semantic_version(uint32_t major, uint32_t minor, uint32_t patch);
uint32_t oracle_make_semantic_version(uint32_t major, uint32_t minor, uint32_t patch)
{
	return MAKE_SEMANTIC_VERSION(major, minor, patch);
}

void oracle_version_layout(size_t *layout);
void oracle_version_layout(size_t *layout)
{
	layout[0] = sizeof(oracle_obs_get_version());
	layout[1] = sizeof(oracle_obs_get_version_string());
	layout[2] = _Alignof(const char *);
}
