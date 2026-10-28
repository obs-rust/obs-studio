/* SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
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
	return (*key && (strcmp(monitor->device_id, key) == 0 || strcmp(monitor->alt_id, key) == 0)) ||
	       (*edid_identity && strcmp(monitor->edid_identity, edid_identity) == 0);
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

/* A connection path is not a serial. Decode the actual EDID base block;
 * monitors without a usable serial cannot safely use the identity fallback. */
static inline bool monitor_edid_identity(const uint8_t *edid, size_t size, char identity[64])
{
	static const uint8_t header[8] = {0, 255, 255, 255, 255, 255, 255, 0};
	identity[0] = 0;
	if (size < 128 || memcmp(edid, header, sizeof(header)) != 0)
		return false;
	unsigned checksum = 0;
	for (size_t i = 0; i < 128; ++i)
		checksum += edid[i];
	if (checksum & 255)
		return false;

	const unsigned manufacturer = ((unsigned)edid[8] << 8) | edid[9];
	const unsigned product = edid[10] | ((unsigned)edid[11] << 8);
	const uint32_t serial = (uint32_t)edid[12] | ((uint32_t)edid[13] << 8) | ((uint32_t)edid[14] << 16) |
				((uint32_t)edid[15] << 24);
	if (!manufacturer)
		return false;
	if (serial && serial != UINT32_MAX) {
		snprintf(identity, 64, "%04X:%04X:N%08X", manufacturer, product, (unsigned)serial);
		return true;
	}
	for (size_t offset = 54; offset + 18 <= 126; offset += 18) {
		const uint8_t *descriptor = edid + offset;
		if (descriptor[0] || descriptor[1] || descriptor[2] || descriptor[3] != 255 || descriptor[4])
			continue;
		char text[14] = {0};
		size_t length = 0;
		for (size_t i = 5; i < 18 && descriptor[i] != 10; ++i) {
			if (descriptor[i] < 32 || descriptor[i] > 126)
				return false;
			text[length++] = (char)descriptor[i];
		}
		while (length && text[length - 1] == ' ')
			text[--length] = 0;
		if (!length)
			return false;
		snprintf(identity, 64, "%04X:%04X:T%s", manufacturer, product, text);
		return true;
	}
	return false;
}
