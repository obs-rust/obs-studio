/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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
******************************************************************************/

#include "libobs/build/libobs-ported-output-protocols.pch.h"

/* Output protocol-list match, moved from obs-output.c: how many times
 * protocol appears in a service output's ';'-separated protocols string.
 * Segments are compared byte for byte, whole and case-sensitive, so "RTMP"
 * does not match "RTMPS" or "rtmp", and an empty segment matches only an
 * empty protocol. A protocol listed twice counts twice. A NULL list counts 0.
 * obs_enum_output_types_with_protocol() in obs-output.c calls enum_cb that
 * many times for the output. Pure: no libobs state. Declared in
 * obs-internal.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-output) replaces this file. */

size_t obs_output_protocol_list_count(const char *protocols, const char *protocol)
{
	size_t protocol_len = strlen(protocol);
	size_t count = 0;

	const char *substr = protocols;
	while (substr && substr[0] != '\0') {
		const char *next = strchr(substr, ';');
		size_t len = next ? (size_t)(next - substr) : strlen(substr);
		if (protocol_len == len && strncmp(substr, protocol, len) == 0)
			count++;
		substr = next ? next + 1 : NULL;
	}

	return count;
}
