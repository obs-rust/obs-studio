#include "libobs/build/libobs.pch.h"

extern const char *get_module_extension(void);

static inline char *get_module_name(const char *file)
{
	return obs_module_names_module_name(file, get_module_extension());
}
