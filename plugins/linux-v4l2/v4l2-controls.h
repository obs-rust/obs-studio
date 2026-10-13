/*
Copyright (C) 2019 by Gary Kramlich <grim@reaperworld.com>

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

#include <obs-module.h>
#include <util/dstr.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Settings: the camera the saved controls were set on (card and bus_info) */
#define V4L2_CONTROLS_DEVICE "controls_device"
/** Settings: the controls saved for the other cameras the source used */
#define V4L2_CONTROLS_OTHER_DEVICES "controls_other_devices"

/**
 * Update a devices properties to include all v4l controls that it supports.
 *
 * @param dev handle to the v4l2 device
 * @param props properties for the device
 * @param data the settings for the source
 *
 * @return negative on failure
 */
int_fast32_t v4l2_update_controls(int_fast32_t dev, obs_properties_t *props, obs_data_t *settings);

/**
 * Apply the saved camera controls to a device.
 *
 * Writes the controls that have a user value in the settings and differ from
 * the device's current value. Controls without a user value and write-only
 * controls are never written.
 *
 * @param dev handle to the v4l2 device
 * @param settings the settings of the source
 */
void v4l2_apply_controls(int_fast32_t dev, obs_data_t *settings);

/**
 * Get the identity of a device: its card name and bus info, as in the
 * device list. Two cameras of the same model differ by their bus info.
 *
 * @param dev handle to the v4l2 device
 * @param identity set to the identity, empty if the device cannot tell
 */
void v4l2_device_identity(int_fast32_t dev, struct dstr *identity);

/**
 * Make the saved controls in the settings those of a device.
 *
 * Without a device recorded (settings saved before the controls were tied
 * to a device), the saved controls become the device's. Otherwise, if the
 * controls belong to another device, they are put aside for that device and
 * the ones put aside for this device, if any, come back. Nothing is written
 * to the device.
 *
 * @param dev handle to the v4l2 device
 * @param settings the settings of the source
 * @param saved_for the device the saved controls belong to, empty for none
 * @param identity the device's identity (v4l2_device_identity())
 *
 * @return true if the settings changed
 */
bool v4l2_controls_use_device(int_fast32_t dev, obs_data_t *settings, const char *saved_for, const char *identity);

#ifdef __cplusplus
}
#endif
