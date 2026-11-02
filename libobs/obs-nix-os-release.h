#pragma once

#include <stdio.h>
#include <stdlib.h>
#include "util/dstr.h"
#include "obs-nix-system-info.h"

static inline void copy_os_release_value(struct dstr *value, const char *start)
{
	size_t len;
	start = obs_nix_system_info_os_release_value(start, &len);
	if (len)
		dstr_ncopy(value, start, len);
}

static inline void read_distribution_info(FILE *fp, struct dstr *distro, struct dstr *version)
{
	char *line = NULL;
	size_t linecap = 0;
	dstr_init_copy(distro, "Unknown");
	dstr_init(version);
	struct dstr build_id;
	dstr_init(&build_id);
	while (getline(&line, &linecap, fp) != -1) {
		enum obs_nix_os_release_key key;
		const char *start = obs_nix_system_info_os_release_field(line, &key);
		if (!start)
			continue;

		if (key == OBS_NIX_OS_RELEASE_NAME)
			copy_os_release_value(distro, start);
		else if (key == OBS_NIX_OS_RELEASE_VERSION_ID)
			copy_os_release_value(version, start);
		else
			copy_os_release_value(&build_id, start);
	}
	if (!version->len) {
		const char *fallback = obs_nix_system_info_version_fallback(build_id.len != 0, distro->array);
		if (!fallback)
			dstr_copy_dstr(version, &build_id);
		else
			dstr_copy(version, fallback);
	}
	dstr_free(&build_id);
	free(line);
}
