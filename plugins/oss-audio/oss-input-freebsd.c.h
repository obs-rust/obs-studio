/*
Copyright (C) 2020. Ka Ho Ng <khng300@gmail.com>
Copyright (C) 2020. Ed Maste <emaste@freebsd.org>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 2 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

#include "plugins/oss-audio/build/oss-audio-freebsd.pch.h"

#define blog(level, msg, ...) blog(level, "oss-audio: " msg, ##__VA_ARGS__)

#define OSS_SNDSTAT_PATH "/dev/sndstat"

#define OSS_DEVICE_BEGIN "Installed devices:"
#define OSS_USERDEVICE_BEGIN "Installed devices from userspace:"
#define OSS_FV_BEGIN "File Versions:"

void oss_sndstat_add_devices(obs_property_t *p, const char *sndstat_path)
{
	char *line = NULL;
	size_t linecap = 0;
	FILE *fp;
	bool ud_matching = false;
	bool skipall = false;

	fp = fopen(sndstat_path, "r");
	if (fp == NULL) {
		blog(LOG_ERROR, "Failed to open sndstat at '%s'.", sndstat_path);
		return;
	}

	while (getline(&line, &linecap, fp) > 0) {
		int pcm;
		char *ptr, *pdesc, *pmode;
		char *descr = NULL, *devname = NULL;
		char *udname = NULL;

		if (!strncmp(line, OSS_FV_BEGIN, strlen(OSS_FV_BEGIN))) {
			skipall = true;
			continue;
		}
		if (!strncmp(line, OSS_DEVICE_BEGIN, strlen(OSS_DEVICE_BEGIN))) {
			ud_matching = false;
			skipall = false;
			continue;
		}
		if (!strncmp(line, OSS_USERDEVICE_BEGIN, strlen(OSS_USERDEVICE_BEGIN))) {
			ud_matching = true;
			skipall = false;
			continue;
		}
		if (skipall || isblank(line[0]))
			continue;

		if (!ud_matching) {
			if (sscanf(line, "pcm%i: ", &pcm) != 1)
				continue;
		} else {
			char *end = strchr(line, ':');
			if (end == NULL || end - line == 0)
				continue;

			udname = strndup(line, (size_t)(end - line));
			if (udname == NULL)
				continue;
		}
		if ((ptr = strchr(line, '<')) == NULL)
			goto free_all_str;
		pdesc = ptr + 1;
		if ((ptr = strrchr(pdesc, '>')) == NULL)
			goto free_all_str;
		*ptr++ = '\0';
		if ((pmode = strchr(ptr, '(')) == NULL)
			goto free_all_str;
		pmode++;
		if ((ptr = strrchr(pmode, ')')) == NULL)
			goto free_all_str;
		*ptr++ = '\0';
		if (!isdigit(pmode[0])) {
			if (strcmp(pmode, "rec") != 0 && strcmp(pmode, "play/rec") != 0)
				goto free_all_str;
		} else {
			int npcs, nrcs;
			if (sscanf(pmode, "%dp:%*dv/%dr:%*dv", &npcs, &nrcs) != 2)
				goto free_all_str;
			if (nrcs < 1)
				goto free_all_str;
		}
		if (!ud_matching) {
			if (asprintf(&descr, "pcm%i: %s", pcm, pdesc) == -1)
				goto free_all_str;
			if (asprintf(&devname, "/dev/dsp%i", pcm) == -1)
				goto free_all_str;
		} else {
			if (asprintf(&descr, "%s: %s", udname, pdesc) == -1)
				goto free_all_str;
			if (asprintf(&devname, "/dev/%s", udname) == -1)
				goto free_all_str;
		}

		obs_property_list_add_string(p, descr, devname);

	free_all_str:
		free(descr);
		free(devname);
		free(udname);
	}
	free(line);

	fclose(fp);
}

void oss_platform_add_devices(obs_property_t *p)
{
	oss_sndstat_add_devices(p, OSS_SNDSTAT_PATH);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef blog
#undef OSS_SNDSTAT_PATH
#undef OSS_DEVICE_BEGIN
#undef OSS_USERDEVICE_BEGIN
#undef OSS_FV_BEGIN
