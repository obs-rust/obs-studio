#include "menu-text.h"

/* Qt reads '&' in menu text as a mnemonic marker ("&&" shows one '&'), so
 * double every '&' for the name to show as typed. */
size_t menu_text_from_name(char *dst, size_t dst_size, const char *name)
{
	size_t len = 0;

	for (const char *c = name; *c; c++) {
		for (int i = 0; i < (*c == '&' ? 2 : 1); i++) {
			if (dst && len + 1 < dst_size)
				dst[len] = *c;
			len++;
		}
	}

	if (dst && dst_size)
		dst[len < dst_size ? len : dst_size - 1] = '\0';
	return len;
}
