#include "plugins/obs-ffmpeg/build/obs-ffmpeg-host-linux.pch.h"

/* obs-ffmpeg's Linux host (#853): NVENC needs a non-blacklisted NVIDIA
 * display controller in sysfs and libnvidia-encode. */

static int get_id_from_sys(char *d_name, char *type)
{
	char file_name[1024];
	char *c;
	int id;

	snprintf(file_name, sizeof(file_name), "/sys/bus/pci/devices/%s/%s", d_name, type);
	if ((c = os_quick_read_utf8_file(file_name)) == NULL) {
		return -1;
	}
	id = (int)strtol(c, NULL, 16);
	bfree(c);

	return id;
}

static bool nvenc_device_present(bool (*is_blacklisted)(int device_id))
{
	os_dir_t *dir;
	struct os_dirent *dirent;
	bool available = false;

	if ((dir = os_opendir("/sys/bus/pci/devices")) == NULL) {
		return true;
	}

	while ((dirent = os_readdir(dir)) != NULL) {
		int id;

		if (get_id_from_sys(dirent->d_name, "class") != 0x030000 &&
		    get_id_from_sys(dirent->d_name, "class") != 0x030200) { // 0x030000 = VGA compatible controller
									    // 0x030200 = 3D controller
			continue;
		}

		if (get_id_from_sys(dirent->d_name, "vendor") != 0x10de) { // 0x10de = NVIDIA Corporation
			continue;
		}

		if ((id = get_id_from_sys(dirent->d_name, "device")) > 0 && !is_blacklisted(id)) {
			available = true;
			break;
		}
	}

	os_closedir(dir);
	return available;
}

bool obs_ffmpeg_host_nvenc_available(bool (*is_blacklisted)(int device_id))
{
	if (!nvenc_device_present(is_blacklisted))
		return false;

	void *const lib = os_dlopen("libnvidia-encode.so.1");
	if (!lib)
		return false;
	os_dlclose(lib);
	return true;
}

void obs_ffmpeg_host_load(void) {}

void obs_ffmpeg_host_unload(void) {}
