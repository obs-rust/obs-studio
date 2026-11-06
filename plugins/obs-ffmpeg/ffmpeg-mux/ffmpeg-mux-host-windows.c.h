#include "plugins/obs-ffmpeg/ffmpeg-mux/build/ffmpeg-mux-windows.pch.h"

/* The ffmpeg-mux helper's Windows entry point (#853): no error dialogs, a
 * safe DLL search path, UTF-16 arguments converted to UTF-8 and a binary
 * stdin before the shared muxer runs. */

int wmain(int argc, wchar_t *argv_w[])
{
	char **argv;
	int ret;

	SetErrorMode(SEM_FAILCRITICALERRORS);
	SetSearchPathMode(BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE | BASE_SEARCH_PATH_PERMANENT);
	SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
	SetDllDirectoryW(L"");

	/* The CRT supplies a nonnegative argument count. */
	argv = malloc((size_t)argc * sizeof(char *));
	for (int i = 0; i < argc; i++) {
		size_t len = wcslen(argv_w[i]);
		int size;

		size = WideCharToMultiByte(CP_UTF8, 0, argv_w[i], (int)len, NULL, 0, NULL, NULL);
		/* The UTF-8 byte count is nonnegative (zero on conversion failure).
		 * Keep the existing int addition before the allocation-size conversion. */
		argv[i] = malloc((size_t)(size + 1));
		WideCharToMultiByte(CP_UTF8, 0, argv_w[i], (int)len, argv[i], size + 1, NULL, NULL);
		argv[i][size] = 0;
	}

	_setmode(_fileno(stdin), O_BINARY);

	ret = ffmpeg_mux_main(argc, argv);

	for (int i = 0; i < argc; i++)
		free(argv[i]);
	free(argv);
	return ret;
}
