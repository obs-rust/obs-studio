/*
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

#include "libobs/build/libobs-macos-coreaudio-device-rules.pch.h"

/* The device rules of the CoreAudio monitoring device list
 * (coreaudio-enum-devices.c), moved: whether a device is listed, the id
 * count, the "default" id, the name match and the virtual device check.
 * coreaudio-enum-devices.c keeps every AudioObject call, the CFString
 * conversions, the allocations and the logs. Declared in
 * coreaudio-device-rules.h without EXPORT. */

bool coreaudio_device_listed(bool allow_inputs, uint32_t output_stream_bytes)
{
	return allow_inputs || output_stream_bytes != 0;
}

uint32_t coreaudio_device_count(uint32_t first_size, uint32_t second_size, size_t id_size)
{
	/* The array's size: the second call's size is not read. */
	(void)second_size;
	return (uint32_t)(first_size / id_size);
}

bool coreaudio_device_is_default(const char *id)
{
	return strcmp(id, "default") == 0;
}

bool coreaudio_device_names_match(const char *name1, const char *name2)
{
	return name1 && name2 && strcmp(name1, name2) == 0;
}

bool coreaudio_device_is_input(const char *uid)
{
	return astrstri(uid, "soundflower") == NULL && astrstri(uid, "wavtap") == NULL &&
	       astrstri(uid, "soundsiphon") == NULL && astrstri(uid, "ishowu") == NULL &&
	       astrstri(uid, "blackhole") == NULL && astrstri(uid, "loopback") == NULL &&
	       astrstri(uid, "groundcontrol") == NULL && astrstri(uid, "vbcable") == NULL;
}
