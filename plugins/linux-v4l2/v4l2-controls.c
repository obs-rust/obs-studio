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

#include <errno.h>
#include <fcntl.h>
#include <string.h>

#include <linux/videodev2.h>
#include <libv4l2.h>

#include "v4l2-controls.h"

#define blog(level, msg, ...) blog(level, "v4l2-controls: " msg, ##__VA_ARGS__)

#if defined(__LP64__)
#define UINT_TO_POINTER(val) ((void *)(unsigned long)(val))
#define POINTER_TO_UINT(p) ((unsigned int)(unsigned long)(p))
#else
#define UINT_TO_POINTER(val) ((void *)(unsigned int)(val))
#define POINTER_TO_UINT(p) ((unsigned int)(unsigned int)(p))
#endif

#define INVALID_CONTROL_FLAGS (V4L2_CTRL_FLAG_DISABLED | V4L2_CTRL_FLAG_READ_ONLY | V4L2_CTRL_FLAG_VOLATILE)

static inline bool valid_control(struct v4l2_queryctrl *qctrl)
{
	return (qctrl->flags & INVALID_CONTROL_FLAGS) == 0;
}

/*
 * A control is inactive while its automatic mode is on (Exposure Time,
 * Absolute with Auto Exposure in Aperture Priority Mode, White Balance
 * Temperature with White Balance, Automatic): the driver ignores or refuses
 * it (uvcvideo with EACCES). Show it disabled. Returns whether a property's
 * state changed, so the Properties window has to be rebuilt.
 */
static bool v4l2_update_controls_active(int_fast32_t dev, obs_properties_t *props)
{
	struct v4l2_queryctrl qctrl;
	bool changed = false;

	memset(&qctrl, 0, sizeof(qctrl));
	qctrl.id = V4L2_CTRL_FLAG_NEXT_CTRL;
	while (0 == v4l2_ioctl(dev, VIDIOC_QUERYCTRL, &qctrl)) {
		obs_property_t *prop = valid_control(&qctrl) ? obs_properties_get(props, (char *)qctrl.name) : NULL;
		bool active = (qctrl.flags & V4L2_CTRL_FLAG_INACTIVE) == 0;

		if (prop && obs_property_enabled(prop) != active) {
			obs_property_set_enabled(prop, active);
			changed = true;
		}
		qctrl.id |= V4L2_CTRL_FLAG_NEXT_CTRL;
	}

	return changed;
}

/*
 * A control changed in the Properties window. Rebuilding the window only
 * when the change turned another control on or off: a rebuild destroys the
 * slider being dragged (obs-rust/obs-studio#827).
 */
static bool v4l2_control_changed(void *data, obs_properties_t *props, obs_property_t *prop, obs_data_t *settings)
{
	int dev = v4l2_open(obs_data_get_string(settings, "device_id"), O_RDWR | O_NONBLOCK | O_CLOEXEC);
	struct v4l2_queryctrl qctrl;
	struct v4l2_control control;

	if (dev == -1)
		return false;

	control.id = POINTER_TO_UINT(data);

	switch (obs_property_get_type(prop)) {
	case OBS_PROPERTY_BOOL:
		control.value = obs_data_get_bool(settings, obs_property_name(prop));
		break;
	case OBS_PROPERTY_INT:
	case OBS_PROPERTY_LIST:
		control.value = obs_data_get_int(settings, obs_property_name(prop));
		break;
	default:
		blog(LOG_ERROR, "unknown property type for %s", obs_property_name(prop));
		v4l2_close(dev);
		return false;
	}

	/* An inactive control is not written: the driver would refuse it,
	 * and its value is kept for when its automatic mode is turned off. */
	memset(&qctrl, 0, sizeof(qctrl));
	qctrl.id = control.id;
	bool inactive = 0 == v4l2_ioctl(dev, VIDIOC_QUERYCTRL, &qctrl) && (qctrl.flags & V4L2_CTRL_FLAG_INACTIVE);

	if (!inactive && 0 != v4l2_ioctl(dev, VIDIOC_S_CTRL, &control))
		blog(LOG_WARNING, "unable to set %s to %d: %s", obs_property_name(prop), control.value,
		     strerror(errno));

	bool changed = v4l2_update_controls_active(dev, props);

	v4l2_close(dev);

	return changed;
}

static bool v4l2_update_controls_menu(int_fast32_t dev, obs_properties_t *props, struct v4l2_queryctrl *qctrl)
{
	obs_property_t *prop;
	struct v4l2_querymenu qmenu;

	prop = obs_properties_add_list(props, (char *)qctrl->name, (char *)qctrl->name, OBS_COMBO_TYPE_LIST,
				       OBS_COMBO_FORMAT_INT);

	obs_property_set_modified_callback2(prop, v4l2_control_changed, UINT_TO_POINTER(qctrl->id));

	memset(&qmenu, 0, sizeof(qmenu));
	qmenu.id = qctrl->id;

	for (qmenu.index = qctrl->minimum; qmenu.index <= (uint32_t)qctrl->maximum; qmenu.index += qctrl->step) {
		if (0 == v4l2_ioctl(dev, VIDIOC_QUERYMENU, &qmenu)) {
			obs_property_list_add_int(prop, (char *)qmenu.name, qmenu.index);
		}
	}

	if (obs_property_list_item_count(prop) == 0) {
		obs_properties_remove_by_name(props, (char *)qctrl->name);
		return false;
	}

	return true;
}

static inline void add_control_property(obs_properties_t *props, obs_data_t *settings, int_fast32_t dev,
					struct v4l2_queryctrl *qctrl)
{
	obs_property_t *prop = NULL;

	if (!valid_control(qctrl)) {
		return;
	}

	switch (qctrl->type) {
	case V4L2_CTRL_TYPE_INTEGER:
		prop = obs_properties_add_int_slider(props, (char *)qctrl->name, (char *)qctrl->name, qctrl->minimum,
						     qctrl->maximum, qctrl->step);
		obs_data_set_default_int(settings, (char *)qctrl->name, qctrl->default_value);
		obs_property_set_modified_callback2(prop, v4l2_control_changed, UINT_TO_POINTER(qctrl->id));
		break;
	case V4L2_CTRL_TYPE_BOOLEAN:
		prop = obs_properties_add_bool(props, (char *)qctrl->name, (char *)qctrl->name);
		obs_data_set_default_bool(settings, (char *)qctrl->name, qctrl->default_value);
		obs_property_set_modified_callback2(prop, v4l2_control_changed, UINT_TO_POINTER(qctrl->id));
		break;
	case V4L2_CTRL_TYPE_MENU:
	case V4L2_CTRL_TYPE_INTEGER_MENU:
		if (v4l2_update_controls_menu(dev, props, qctrl)) {
			prop = obs_properties_get(props, (char *)qctrl->name);
			obs_data_set_default_int(settings, (char *)qctrl->name, qctrl->default_value);
			blog(LOG_INFO, "setting default for %s to %d", (char *)qctrl->name, qctrl->default_value);
		}
		break;
	}

	if (prop)
		obs_property_set_enabled(prop, (qctrl->flags & V4L2_CTRL_FLAG_INACTIVE) == 0);
}

int_fast32_t v4l2_update_controls(int_fast32_t dev, obs_properties_t *props, obs_data_t *settings)
{
	struct v4l2_queryctrl qctrl;

	if (!dev || !props)
		return -1;

	memset(&qctrl, 0, sizeof(qctrl));
	qctrl.id = V4L2_CTRL_FLAG_NEXT_CTRL;
	while (0 == v4l2_ioctl(dev, VIDIOC_QUERYCTRL, &qctrl)) {
		add_control_property(props, settings, dev, &qctrl);
		qctrl.id |= V4L2_CTRL_FLAG_NEXT_CTRL;
	}

	return 0;
}

/* The value the user saved for a control, if any */
static bool v4l2_saved_control_value(obs_data_t *settings, const struct v4l2_queryctrl *qctrl, int32_t *value)
{
	const char *name = (const char *)qctrl->name;

	if (!obs_data_has_user_value(settings, name))
		return false;

	switch (qctrl->type) {
	case V4L2_CTRL_TYPE_BOOLEAN:
		*value = obs_data_get_bool(settings, name);
		return true;
	case V4L2_CTRL_TYPE_INTEGER:
	case V4L2_CTRL_TYPE_MENU:
	case V4L2_CTRL_TYPE_INTEGER_MENU:
		*value = (int32_t)obs_data_get_int(settings, name);
		return true;
	default:
		return false;
	}
}

/*
 * Write the saved controls that differ from the device's values. Returns how
 * many were left for a later pass: inactive ones, and on the first pass the
 * ones the device refused.
 */
static int v4l2_apply_controls_pass(int_fast32_t dev, obs_data_t *settings, bool last_pass)
{
	struct v4l2_queryctrl qctrl;
	int left = 0;

	memset(&qctrl, 0, sizeof(qctrl));
	qctrl.id = V4L2_CTRL_FLAG_NEXT_CTRL;
	while (0 == v4l2_ioctl(dev, VIDIOC_QUERYCTRL, &qctrl)) {
		struct v4l2_control control = {.id = qctrl.id};
		int32_t value;

		/* A write-only control (Pan, Relative...) acts each time it is
		 * written, and its value cannot be read back: never replay it. */
		if (!valid_control(&qctrl) || (qctrl.flags & V4L2_CTRL_FLAG_WRITE_ONLY) ||
		    !v4l2_saved_control_value(settings, &qctrl, &value))
			goto next;

		if (0 == v4l2_ioctl(dev, VIDIOC_G_CTRL, &control) && control.value == value)
			goto next;

		/* A manual control is inactive while its automatic mode is on,
		 * and drivers can list it first (Focus (absolute) comes before
		 * Focus, Automatic Continuous): retry it after the others. */
		if (qctrl.flags & V4L2_CTRL_FLAG_INACTIVE) {
			left++;
			goto next;
		}

		control.value = value;
		if (0 != v4l2_ioctl(dev, VIDIOC_S_CTRL, &control)) {
			if (!last_pass)
				left++;
			else
				blog(LOG_WARNING, "unable to set %s to %d: %s", (char *)qctrl.name, value,
				     strerror(errno));
		}

	next:
		qctrl.id |= V4L2_CTRL_FLAG_NEXT_CTRL;
	}

	return left;
}

void v4l2_apply_controls(int_fast32_t dev, obs_data_t *settings)
{
	if (dev < 0 || !settings)
		return;

	if (v4l2_apply_controls_pass(dev, settings, false) > 0)
		v4l2_apply_controls_pass(dev, settings, true);
}

void v4l2_device_identity(int_fast32_t dev, struct dstr *identity)
{
	struct v4l2_capability cap;

	dstr_free(identity);
	memset(&cap, 0, sizeof(cap));
	if (dev < 0 || 0 != v4l2_ioctl(dev, VIDIOC_QUERYCAP, &cap))
		return;

	/* Bounded: the kernel's strings fill their arrays at most */
	dstr_printf(identity, "%.*s (%.*s)", (int)sizeof(cap.card), (char *)cap.card, (int)sizeof(cap.bus_info),
		    (char *)cap.bus_info);
}

/* Move a control's user value, if it has one, from one settings to another */
static void v4l2_move_control_value(obs_data_t *from, obs_data_t *to, const char *name)
{
	obs_data_item_t *item = obs_data_item_byname(from, name);

	if (!item)
		return;

	if (obs_data_item_has_user_value(item)) {
		switch (obs_data_item_gettype(item)) {
		case OBS_DATA_BOOLEAN:
			obs_data_set_bool(to, name, obs_data_item_get_bool(item));
			break;
		case OBS_DATA_NUMBER:
			obs_data_set_int(to, name, obs_data_item_get_int(item));
			break;
		default:
			break;
		}
		obs_data_item_unset_user_value(item);
	}
	obs_data_item_release(&item);
}

bool v4l2_controls_use_device(int_fast32_t dev, obs_data_t *settings, const char *saved_for, const char *identity)
{
	if (!settings || !identity || !*identity)
		return false;
	if (saved_for && strcmp(saved_for, identity) == 0) {
		/* Settings that only had it in memory record it */
		if (strcmp(obs_data_get_string(settings, V4L2_CONTROLS_DEVICE), identity) == 0)
			return false;
		obs_data_set_string(settings, V4L2_CONTROLS_DEVICE, identity);
		return true;
	}

	if (saved_for && *saved_for) {
		obs_data_t *others = obs_data_get_obj(settings, V4L2_CONTROLS_OTHER_DEVICES);
		obs_data_t *theirs = obs_data_create();
		struct v4l2_queryctrl qctrl;

		if (!others)
			others = obs_data_create();

		/* The controls of this device's names are the other device's:
		 * put them aside for it. */
		memset(&qctrl, 0, sizeof(qctrl));
		qctrl.id = V4L2_CTRL_FLAG_NEXT_CTRL;
		while (0 == v4l2_ioctl(dev, VIDIOC_QUERYCTRL, &qctrl)) {
			v4l2_move_control_value(settings, theirs, (const char *)qctrl.name);
			qctrl.id |= V4L2_CTRL_FLAG_NEXT_CTRL;
		}
		obs_data_item_t *first = obs_data_first(theirs);
		if (first) {
			obs_data_item_release(&first);
			obs_data_set_obj(others, saved_for, theirs);
		}
		obs_data_release(theirs);

		/* Bring back the ones put aside for this device */
		obs_data_t *mine = obs_data_get_obj(others, identity);
		if (mine) {
			obs_data_apply(settings, mine);
			obs_data_erase(others, identity);
			obs_data_release(mine);
		}

		obs_data_set_obj(settings, V4L2_CONTROLS_OTHER_DEVICES, others);
		obs_data_release(others);
	}

	obs_data_set_string(settings, V4L2_CONTROLS_DEVICE, identity);
	return true;
}
