#include "frontend/build/frontend-utility-profile-activate.pch.h"

void profile_activate_config(const struct profile_activate_ops *ops, bool reset)
{
	ops->init_defaults(ops->param);

	/* Defaults live in the loaded configuration only, and a renamed or
	 * duplicated profile is a new copy of basic.ini: set the encoder
	 * defaults on every activation, or keys the profile never saved read
	 * as NULL. */
	ops->init_encoder_defaults(ops->param);

	if (reset) {
		ops->check_encoders(ops->param);
		ops->reset_data(ops->param);
	}
}

void profile_set_encoder_defaults(config_t *config, const char *simple_encoder, const char *aac_encoder)
{
	config_set_default_string(config, "SimpleOutput", "StreamEncoder", simple_encoder);
	config_set_default_string(config, "SimpleOutput", "RecEncoder", simple_encoder);

	config_set_default_string(config, "AdvOut", "AudioEncoder", aac_encoder);
	config_set_default_string(config, "AdvOut", "RecAudioEncoder", aac_encoder);
}

static bool profile_encoder_id_in(const char *id, const char *const *ids)
{
	for (; *ids; ids++) {
		const char *a = id;
		const char *b = *ids;
		while (*a && *a == *b) {
			a++;
			b++;
		}
		if (*a == '\0' && *b == '\0') {
			return true;
		}
	}
	return false;
}

/* The encoders plugins/obs-qsv11 and plugins/obs-ffmpeg's VA-API encoder
 * register, by id. */
static const char *const profile_qsv_encoder_ids[] = {
	"obs_qsv11",         "obs_qsv11_v2",       "obs_qsv11_soft",
	"obs_qsv11_soft_v2", "obs_qsv11_hevc",     "obs_qsv11_hevc_soft",
	"obs_qsv11_av1",     "obs_qsv11_av1_soft", NULL,
};
static const char *const profile_vaapi_encoder_ids[] = {
	"ffmpeg_vaapi",
	"ffmpeg_vaapi_tex",
	"hevc_ffmpeg_vaapi",
	"hevc_ffmpeg_vaapi_tex",
	"av1_ffmpeg_vaapi",
	"av1_ffmpeg_vaapi_tex",
	NULL,
};

enum profile_missing_encoder
profile_missing_encoder_reason(const char *encoder_id, bool (*module_loaded)(const char *name), const char **module)
{
	const char *name = NULL;
	enum profile_missing_encoder no_device = PROFILE_MISSING_ENCODER_UNKNOWN;

	if (encoder_id && profile_encoder_id_in(encoder_id, profile_qsv_encoder_ids)) {
		name = "obs-qsv11";
		no_device = PROFILE_MISSING_ENCODER_NO_QSV_DEVICE;
	} else if (encoder_id && profile_encoder_id_in(encoder_id, profile_vaapi_encoder_ids)) {
		name = "obs-ffmpeg";
		no_device = PROFILE_MISSING_ENCODER_NO_VAAPI_DEVICE;
	}

	if (module) {
		*module = name;
	}
	if (!name) {
		return PROFILE_MISSING_ENCODER_UNKNOWN;
	}
	return module_loaded(name) ? no_device : PROFILE_MISSING_ENCODER_MODULE_NOT_LOADED;
}

static bool profile_vaapi_read_file(void *param, const char *path, char *buf, size_t size)
{
	(void)param;
	FILE *f = fopen(path, "rb");
	size_t n;
	if (!f) {
		return false;
	}
	n = fread(buf, 1, size - 1, f);
	buf[n] = '\0';
	fclose(f);
	return true;
}

static bool profile_vaapi_exists(void *param, const char *path)
{
	(void)param;
	FILE *f = fopen(path, "rb");
	if (!f) {
		return false;
	}
	fclose(f);
	return true;
}

static bool profile_vaapi_can_open_rw(void *param, const char *path)
{
	(void)param;
	FILE *f = fopen(path, "r+b");
	if (!f) {
		return false;
	}
	fclose(f);
	return true;
}

const struct profile_vaapi_host profile_vaapi_system_host = {
	NULL,
	profile_vaapi_read_file,
	profile_vaapi_exists,
	profile_vaapi_can_open_rw,
};

#define PROFILE_PCI_VENDOR_INTEL 0x8086u
#define PROFILE_PCI_VENDOR_AMD 0x1002u
#define PROFILE_PCI_VENDOR_NVIDIA 0x10deu

/* libva's render device range, and where distributions install VA-API
 * drivers (hostlib's search dirs, build-aux/appimage/hostlib/hostlib.c). */
#define PROFILE_RENDER_NODE_FIRST 128
#define PROFILE_RENDER_NODE_COUNT 8

static const char *const profile_vaapi_driver_dirs[] = {
	"/run/opengl-driver/lib/dri",
	"/usr/lib/x86_64-linux-gnu/dri",
	"/usr/lib/aarch64-linux-gnu/dri",
	"/usr/lib64/dri",
	"/usr/lib/dri",
	NULL,
};

enum profile_distro {
	PROFILE_DISTRO_UNKNOWN,
	PROFILE_DISTRO_DEBIAN,
	PROFILE_DISTRO_FEDORA,
	PROFILE_DISTRO_ARCH,
};

struct profile_vaapi_vendor {
	unsigned int vendor;
	const char *const drivers[3];
	/* By enum profile_distro; [PROFILE_DISTRO_UNKNOWN] is the name shown
	 * when the distribution is not recognized. */
	const char *packages[4];
};

static const struct profile_vaapi_vendor profile_vaapi_vendors[] = {
	{PROFILE_PCI_VENDOR_INTEL,
	 {"iHD", "i965", NULL},
	 {"intel-media-driver", "intel-media-va-driver-non-free", "intel-media-driver", "intel-media-driver"}},
	{PROFILE_PCI_VENDOR_AMD,
	 {"radeonsi", NULL, NULL},
	 {"mesa-va-drivers", "mesa-va-drivers", "mesa-va-drivers-freeworld", "mesa"}},
};

static const char *const profile_install_prefix[] = {NULL, "sudo apt install ", "sudo dnf install ", "sudo pacman -S "};

/* Whether the space-separated, optionally quoted value of key (ID or
 * ID_LIKE) in os-release text names id. */
static bool profile_os_release_has(const char *text, const char *key, const char *id)
{
	size_t key_len = strlen(key);
	size_t id_len = strlen(id);
	const char *line = text;

	while (line && *line) {
		if (strncmp(line, key, key_len) == 0 && line[key_len] == '=') {
			const char *p = line + key_len + 1;
			while (*p && *p != '\n') {
				while (*p == ' ' || *p == '"' || *p == '\'') {
					p++;
				}
				const char *word = p;
				while (*p && *p != '\n' && *p != ' ' && *p != '"' && *p != '\'') {
					p++;
				}
				if ((size_t)(p - word) == id_len && strncmp(word, id, id_len) == 0) {
					return true;
				}
				if (*p == '"' || *p == '\'') {
					p++;
				}
			}
		}
		line = strchr(line, '\n');
		if (line) {
			line++;
		}
	}
	return false;
}

static enum profile_distro profile_detect_distro(const struct profile_vaapi_host *host)
{
	static const struct {
		const char *id;
		enum profile_distro distro;
	} ids[] = {
		{"debian", PROFILE_DISTRO_DEBIAN}, {"ubuntu", PROFILE_DISTRO_DEBIAN}, {"fedora", PROFILE_DISTRO_FEDORA},
		{"rhel", PROFILE_DISTRO_FEDORA},   {"arch", PROFILE_DISTRO_ARCH},
	};
	char text[2048];

	if (!host->read_file(host->param, "/etc/os-release", text, sizeof(text))) {
		return PROFILE_DISTRO_UNKNOWN;
	}
	for (size_t k = 0; k < 2; k++) {
		const char *key = k == 0 ? "ID" : "ID_LIKE";
		for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
			if (profile_os_release_has(text, key, ids[i].id)) {
				return ids[i].distro;
			}
		}
	}
	return PROFILE_DISTRO_UNKNOWN;
}

static unsigned int profile_read_vendor(const struct profile_vaapi_host *host, int node)
{
	char path[64];
	char text[32];

	snprintf(path, sizeof(path), "/sys/class/drm/renderD%d/device/vendor", node);
	if (!host->read_file(host->param, path, text, sizeof(text))) {
		return 0;
	}
	return (unsigned int)strtoul(text, NULL, 16);
}

static bool profile_vaapi_driver_installed(const struct profile_vaapi_host *host, const struct profile_vaapi_vendor *v)
{
	char path[128];

	for (const char *const *d = v->drivers; *d; d++) {
		for (const char *const *dir = profile_vaapi_driver_dirs; *dir; dir++) {
			snprintf(path, sizeof(path), "%s/%s_drv_video.so", *dir, *d);
			if (host->exists(host->param, path)) {
				return true;
			}
		}
	}
	return false;
}

void profile_vaapi_diagnose(const struct profile_vaapi_host *host, struct profile_vaapi_diagnosis *out)
{
	static char install_command[96];
	bool found_nvidia = false;
	int node = -1;

	memset(out, 0, sizeof(*out));

	/* The first render device that is not an NVIDIA GPU: libva's DRM
	 * path and obs-ffmpeg try those, NVIDIA has no VA-API encoder. */
	for (int i = 0; i < PROFILE_RENDER_NODE_COUNT; i++) {
		char path[32];
		unsigned int vendor;

		snprintf(path, sizeof(path), "/dev/dri/renderD%d", PROFILE_RENDER_NODE_FIRST + i);
		if (!host->exists(host->param, path)) {
			continue;
		}
		vendor = profile_read_vendor(host, PROFILE_RENDER_NODE_FIRST + i);
		if (vendor == PROFILE_PCI_VENDOR_NVIDIA) {
			if (!found_nvidia) {
				snprintf(out->device, sizeof(out->device), "%s", path);
				out->vendor = vendor;
			}
			found_nvidia = true;
			continue;
		}
		snprintf(out->device, sizeof(out->device), "%s", path);
		out->vendor = vendor;
		node = PROFILE_RENDER_NODE_FIRST + i;
		break;
	}

	if (node < 0) {
		out->cause = found_nvidia ? PROFILE_VAAPI_NVIDIA : PROFILE_VAAPI_NO_RENDER_NODE;
		return;
	}
	if (!host->can_open_rw(host->param, out->device)) {
		out->cause = PROFILE_VAAPI_NO_ACCESS;
		return;
	}

	out->cause = PROFILE_VAAPI_NO_ENCODE;
	for (size_t i = 0; i < sizeof(profile_vaapi_vendors) / sizeof(profile_vaapi_vendors[0]); i++) {
		const struct profile_vaapi_vendor *v = &profile_vaapi_vendors[i];
		enum profile_distro distro;

		if (v->vendor != out->vendor || profile_vaapi_driver_installed(host, v)) {
			continue;
		}
		distro = profile_detect_distro(host);
		out->cause = PROFILE_VAAPI_NO_DRIVER;
		out->package = v->packages[distro];
		if (profile_install_prefix[distro]) {
			snprintf(install_command, sizeof(install_command), "%s%s", profile_install_prefix[distro],
				 out->package);
			out->install_command = install_command;
		}
		break;
	}
}

#undef PROFILE_PCI_VENDOR_INTEL
#undef PROFILE_PCI_VENDOR_AMD
#undef PROFILE_PCI_VENDOR_NVIDIA
#undef PROFILE_RENDER_NODE_FIRST
#undef PROFILE_RENDER_NODE_COUNT
