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

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The device rules of the CoreAudio monitoring device list
 * (coreaudio-enum-devices.c), on plain values and C strings
 * (coreaudio-device-rules.c):
 * - obs_enum_audio_monitoring_device(): coreaudio_device_listed();
 * - enum_audio_devices(): coreaudio_device_count();
 * - devices_match(): coreaudio_device_is_default() and
 *   coreaudio_device_names_match();
 * - get_desktop_default_id() and find_loopback_cb():
 *   coreaudio_device_is_input().
 *
 * coreaudio-enum-devices.c keeps every AudioObject call, the CFString
 * conversions, every allocation and free, the logs and the callbacks, and
 * reads each value where it did. Nothing here includes an Apple header, so
 * the C tests build it on every platform. Not EXPORT: only libobs calls
 * these. */

/* Whether the device list shows a device whose output stream list is
 * output_stream_bytes long: always with inputs, else only a device with
 * output streams. */
bool coreaudio_device_listed(bool allow_inputs, uint32_t output_stream_bytes);

/* How many device ids of id_size bytes enum_audio_devices() walks, given
 * the size the first call reported (the array's size) and the size the
 * second call wrote. */
uint32_t coreaudio_device_count(uint32_t first_size, uint32_t second_size, size_t id_size);

/* Whether a monitoring device id names the default output. */
bool coreaudio_device_is_default(const char *id);

/* Whether two devices whose names were looked up match: both names were
 * found and are equal. Devices are matched by name, not uid. */
bool coreaudio_device_names_match(const char *name1, const char *name2);

/* Whether a device uid names a real device: it contains, ignoring ASCII
 * case, none of the virtual device names (Soundflower, WavTap,
 * SoundSiphon, iShowU, BlackHole, Loopback, GroundControl, VB-Cable). */
bool coreaudio_device_is_input(const char *uid);
