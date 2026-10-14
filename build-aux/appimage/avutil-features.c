/* Inspect the actual libavutil loaded from a candidate package. */
#include <dlfcn.h>
#include <stdio.h>

#include <libavutil/hwcontext.h>

int main(int argc, char **argv)
{
	if (argc != 2)
		return 2;
	void *library = dlopen(argv[1], RTLD_NOW);
	if (!library) {
		fprintf(stderr, "%s\n", dlerror());
		return 3;
	}
	unsigned (*version)(void) = (unsigned (*)(void))dlsym(library, "avutil_version");
	enum AVHWDeviceType (*next)(enum AVHWDeviceType) =
		(enum AVHWDeviceType (*)(enum AVHWDeviceType))dlsym(library, "av_hwdevice_iterate_types");
	const char *(*name)(enum AVHWDeviceType) =
		(const char *(*)(enum AVHWDeviceType))dlsym(library, "av_hwdevice_get_type_name");
	if (!version || !next || !name) {
		dlclose(library);
		return 4;
	}
	printf("version=%u\n", version());
	for (enum AVHWDeviceType type = next(AV_HWDEVICE_TYPE_NONE); type != AV_HWDEVICE_TYPE_NONE; type = next(type))
		printf("backend=%s\n", name(type));
	dlclose(library);
	return 0;
}
