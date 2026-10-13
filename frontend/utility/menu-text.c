#include "menu-text.h"

size_t menu_text_from_name(char *dst, size_t dst_size, const char *name)
{
	size_t len = 0;

	for (const char *c = name; *c; c++) {
		if (dst && len + 1 < dst_size)
			dst[len] = *c;
		len++;
	}

	if (dst && dst_size)
		dst[len < dst_size ? len : dst_size - 1] = '\0';
	return len;
}
