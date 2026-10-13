#pragma once

#include <stddef.h>

#ifdef __cplusplus
#include <string>

extern "C" {
#endif

/* The text of a menu entry that shows a name the user typed (a profile or a
 * scene collection), kept free of Qt so it can be unit tested. Qt reads '&'
 * in menu text as a mnemonic marker, so every '&' is doubled for the menu to
 * show the name as typed. Writes the text into dst (NUL-terminated when dst_size is not 0, truncated if needed)
 * and returns the length it needs, without the NUL. */
size_t menu_text_from_name(char *dst, size_t dst_size, const char *name);

#ifdef __cplusplus
}

inline std::string menu_text_from_name(const std::string &name)
{
	std::string text(menu_text_from_name(nullptr, 0, name.c_str()), '\0');
	menu_text_from_name(text.data(), text.size() + 1, name.c_str());
	return text;
}
#endif
