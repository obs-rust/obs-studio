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

#include "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"

#define FORCE_POLL_MS 50
#define RETRY_FIRST_MS 2000
#define RETRY_MAX_MS 30000

bool v4l2_busy_errno(int err)
{
	return err == EBUSY;
}

static bool parse_pid(const char *name, pid_t *pid)
{
	if (!*name)
		return false;
	for (const char *c = name; *c; c++) {
		if (!isdigit((unsigned char)*c))
			return false;
	}
	long value = strtol(name, NULL, 10);
	if (value <= 0 || value > INT_MAX)
		return false;
	*pid = (pid_t)value;
	return true;
}

/* Whether <proc_root>/<pid>/fd has a descriptor on the resolved device */
static bool process_holds(const char *proc_root, pid_t pid, const char *device)
{
	char fd_dir[PATH_MAX];
	snprintf(fd_dir, sizeof(fd_dir), "%s/%d/fd", proc_root, (int)pid);

	DIR *dir = opendir(fd_dir);
	if (!dir)
		return false;

	bool holds = false;
	struct dirent *ent;
	while (!holds && (ent = readdir(dir)) != NULL) {
		if (ent->d_name[0] == '.')
			continue;

		char link[PATH_MAX];
		char target[PATH_MAX];
		int n = snprintf(link, sizeof(link), "%s/%s", fd_dir, ent->d_name);
		if (n < 0 || (size_t)n >= sizeof(link))
			continue;
		ssize_t len = readlink(link, target, sizeof(target) - 1);
		if (len <= 0)
			continue;
		target[len] = 0;
		holds = strcmp(target, device) == 0;
	}
	closedir(dir);
	return holds;
}

static void read_holder(const char *proc_root, pid_t pid, struct v4l2_holder *holder)
{
	char path[PATH_MAX];
	struct stat st;

	memset(holder, 0, sizeof(*holder));
	holder->pid = pid;

	snprintf(path, sizeof(path), "%s/%d", proc_root, (int)pid);
	holder->uid = stat(path, &st) == 0 ? st.st_uid : (uid_t)-1;

	snprintf(path, sizeof(path), "%s/%d/comm", proc_root, (int)pid);
	FILE *f = fopen(path, "r");
	if (f) {
		if (fgets(holder->name, sizeof(holder->name), f))
			holder->name[strcspn(holder->name, "\n")] = 0;
		fclose(f);
	}
	if (!*holder->name)
		snprintf(holder->name, sizeof(holder->name), "?");
}

size_t v4l2_find_holders(const char *proc_root, const char *dev_path, pid_t self_pid, struct v4l2_holder *out,
			 size_t max)
{
	char device[PATH_MAX];
	if (!dev_path || !realpath(dev_path, device))
		return 0;

	DIR *proc = opendir(proc_root);
	if (!proc)
		return 0;

	size_t found = 0;
	struct dirent *ent;
	while (found < max && (ent = readdir(proc)) != NULL) {
		pid_t pid;
		if (!parse_pid(ent->d_name, &pid) || pid == self_pid)
			continue;
		if (process_holds(proc_root, pid, device))
			read_holder(proc_root, pid, &out[found++]);
	}
	closedir(proc);
	return found;
}

bool v4l2_holder_is_obs(const struct v4l2_holder *holder)
{
	/* comm is at most 15 characters: "obs-studio-rust" exactly fits.
	 * Nix wraps the binary as .obs-wrapped; the AppImage starts as AppRun. */
	static const char *names[] = {"obs", ".obs-wrapped", "obs-studio-rust", "AppRun"};

	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
		if (strcmp(holder->name, names[i]) == 0)
			return true;
	}
	return false;
}

void v4l2_busy_describe(struct dstr *out, const char *dev_path, const struct v4l2_holder *holder)
{
	if (holder)
		dstr_printf(out, "%s is in use by %s (PID %d)", dev_path, holder->name, (int)holder->pid);
	else
		dstr_printf(out, "%s is in use by another program", dev_path);
}

enum v4l2_force_result v4l2_force_release(const char *proc_root, const char *dev_path, const struct v4l2_holder *holder,
					  uid_t self_uid, uint32_t timeout_ms)
{
	if (holder->uid != self_uid)
		return V4L2_FORCE_NOT_OWNER;

	char device[PATH_MAX];
	if (!realpath(dev_path, device))
		return V4L2_FORCE_ERROR;

	if (kill(holder->pid, SIGTERM) != 0)
		return errno == ESRCH ? V4L2_FORCE_RELEASED : V4L2_FORCE_ERROR;

	/* An exited child of ours lingers as a zombie, which has no
	 * descriptors left, so this also sees it as released. */
	for (uint32_t waited = 0;; waited += FORCE_POLL_MS) {
		if (!process_holds(proc_root, holder->pid, device))
			return V4L2_FORCE_RELEASED;
		if (waited >= timeout_ms)
			return V4L2_FORCE_TIMEOUT;
		os_sleep_ms(FORCE_POLL_MS);
	}
}

uint32_t v4l2_busy_retry_delay_ms(uint32_t attempt)
{
	if (attempt >= 4)
		return RETRY_MAX_MS;
	uint32_t delay = (uint32_t)RETRY_FIRST_MS << attempt;
	return delay < RETRY_MAX_MS ? delay : RETRY_MAX_MS;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef FORCE_POLL_MS
#undef RETRY_FIRST_MS
#undef RETRY_MAX_MS
