/* Test-only stand-ins for libobs symbols (base.c, platform.c) that the
 * oracles and the Rust bmem shim reference under cargo test. Delete together
 * with the oracles once the C sources are deleted.
 *
 * Deliberately includes no libobs header: they mark these functions
 * EXPORT/dllexport. Prototypes below match libobs/util/base.h and
 * libobs/util/platform.h. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

void *bmalloc(size_t size);
void bfree(void *ptr);

void os_breakpoint(void);
void os_oom(void);
size_t os_fread_utf8(FILE *file, char **pstr);

void os_breakpoint(void) {}

void os_oom(void) {}

/* os_fopen comes from oracle/file_serializer_host.c. */

/* Mirrors platform.c os_fread_utf8, including returning 0 for the length. */
size_t os_fread_utf8(FILE *file, char **pstr)
{
	size_t size = 0;
	size_t len = 0;

	*pstr = NULL;

	fseek(file, 0, SEEK_END);
	size = (size_t)ftell(file);

	if (size > 0) {
		char bom[3] = {0, 0, 0};
		char *utf8str;
		long offset;

		fseek(file, 0, SEEK_SET);
		size_t size_read = fread(bom, 1, 3, file);
		(void)size_read;

		offset = (memcmp(bom, "\xEF\xBB\xBF", 3) == 0) ? 3 : 0;

		size -= (size_t)offset;
		if (size == 0)
			return 0;

		utf8str = bmalloc(size + 1);
		fseek(file, offset, SEEK_SET);

		size = fread(utf8str, 1, size, file);
		if (size == 0) {
			bfree(utf8str);
			return 0;
		}

		utf8str[size] = 0;

		*pstr = utf8str;
	}

	return len;
}
