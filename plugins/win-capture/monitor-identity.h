/* SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

/* Private, portable part of the enumerated Windows monitor. */
struct duplicator_monitor_info {
	char device_id[128];
	char id[128];
	char alt_id[128];
	char name[128];
	char edid_identity[64];
};

static inline void monitor_fill_info(struct duplicator_monitor_info *monitor, const char *device_id,
				     const char *gdi_name, const char *name, const char *edid_identity)
{
	snprintf(monitor->device_id, sizeof(monitor->device_id), "%s", device_id);
	snprintf(monitor->id, sizeof(monitor->id), "%s", device_id);
	snprintf(monitor->alt_id, sizeof(monitor->alt_id), "%s", gdi_name);
	snprintf(monitor->name, sizeof(monitor->name), "%s", name);
	snprintf(monitor->edid_identity, sizeof(monitor->edid_identity), "%s", edid_identity);
}

/* The caller tries the interface key before the GDI-name fallback. */
static inline bool monitor_matches(const struct duplicator_monitor_info *monitor, const char *key,
				   const char *edid_identity)
{
	(void)edid_identity;
	return *key && (strcmp(monitor->device_id, key) == 0 || strcmp(monitor->alt_id, key) == 0);
}

/* Count the complete pass: the first EDID candidate alone is insufficient. */
static inline size_t monitor_find(const struct duplicator_monitor_info *monitors, size_t count, const char *key,
				  const char *edid_identity)
{
	for (size_t i = 0; i < count; ++i)
		if (*key && strcmp(monitors[i].device_id, key) == 0)
			return i;
	for (size_t i = 0; i < count; ++i)
		if (*key && strcmp(monitors[i].alt_id, key) == 0)
			return i;

	size_t found = count;
	for (size_t i = 0; i < count; ++i) {
		if (monitor_matches(&monitors[i], "", edid_identity)) {
			if (found != count)
				return count;
			found = i;
		}
	}
	return found;
}
