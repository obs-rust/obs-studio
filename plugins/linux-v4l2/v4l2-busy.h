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
#include <sys/types.h>

#include <util/dstr.h>

/* A camera another process has open and streaming fails with EBUSY
 * (obs-rust/obs-studio#799). V4L2 has no lock file to break: the "lock" is
 * the other process's open descriptor. These helpers find that process,
 * close it on request and pace the retries, without touching the device, so
 * they can be tested on their own. */

#define V4L2_HOLDER_NAME_LEN 64

/* A process holding the device open */
struct v4l2_holder {
	pid_t pid;
	uid_t uid;
	char name[V4L2_HOLDER_NAME_LEN];
};

enum v4l2_force_result {
	/* the holder exited or closed the device */
	V4L2_FORCE_RELEASED,
	/* the holder belongs to another user: not signalled */
	V4L2_FORCE_NOT_OWNER,
	/* SIGTERM sent, but the holder still has the device open; never
	 * escalated to SIGKILL */
	V4L2_FORCE_TIMEOUT,
	/* the signal could not be sent */
	V4L2_FORCE_ERROR,
};

/* Whether an open/ioctl errno means another process has the device. */
bool v4l2_busy_errno(int err);

/* Find up to max processes other than self_pid with a descriptor on
 * dev_path, by scanning <proc_root>/<pid>/fd (proc_root is "/proc" outside
 * tests). Symlinks such as /dev/v4l/by-id/... are resolved first. Processes
 * whose descriptors cannot be read (other users) are not found. Returns the
 * number found. */
size_t v4l2_find_holders(const char *proc_root, const char *dev_path, pid_t self_pid, struct v4l2_holder *out,
			 size_t max);

/* Whether the holder is another OBS instance, the common case. */
bool v4l2_holder_is_obs(const struct v4l2_holder *holder);

/* "<device> is in use by <name> (PID <pid>)", or a generic message without a
 * holder. */
void v4l2_busy_describe(struct dstr *out, const char *dev_path, const struct v4l2_holder *holder);

/* Ask the holder to close (SIGTERM) and wait up to timeout_ms for it to let
 * go of dev_path. Only a holder owned by self_uid is signalled. */
enum v4l2_force_result v4l2_force_release(const char *proc_root, const char *dev_path, const struct v4l2_holder *holder,
					  uid_t self_uid, uint32_t timeout_ms);

/* Delay before the given retry (0-based) of a busy device, in ms: 2 s,
 * doubling to at most 30 s. */
uint32_t v4l2_busy_retry_delay_ms(uint32_t attempt);

/* Whether a failed open of a device announces it busy: on the first failure
 * of a busy episode (!was_busy), and again the first time a retry finds the
 * holder when no announcement of this episode named one (#3726). */
bool v4l2_busy_should_announce(bool was_busy, bool announced_known, bool known);
