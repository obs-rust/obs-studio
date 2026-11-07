#include "libobs/build/libobs-ported-module-paths.pch.h"

void obs_module_failure_info_free(struct obs_module_failure_info *mfi)
{
	if (mfi->failed_modules) {
		bfree(mfi->failed_modules);
		mfi->failed_modules = NULL;
	}
}
