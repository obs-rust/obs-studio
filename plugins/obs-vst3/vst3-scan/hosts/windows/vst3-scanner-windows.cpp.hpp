// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugins/obs-vst3/vst3-scan/build/obs-vst3-scanner.windows.pch.hpp"

int wmain(int argc, wchar_t *argv[])
{
	SetErrorMode(SEM_FAILCRITICALERRORS);
	SetSearchPathMode(BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE | BASE_SEARCH_PATH_PERMANENT);
	SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
	SetDllDirectoryW(L"");

	char *cachePath = nullptr;
	if (argc > 1) {
		os_wcs_to_utf8_ptr(argv[1], 0, &cachePath);
	}
	const int result = vst3_scanner_main(cachePath);
	bfree(cachePath);
	return result;
}
