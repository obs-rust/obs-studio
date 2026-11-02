/* Test-only oracle: the original libobs/graphics/shader-param-values.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. valid_int_str is the C lexer.c, already renamed in
 * oracle/lexer.c, os_strtod the C platform-number.c, already renamed in
 * oracle/platform_number.c, and strtol and strtod the C library's, so the
 * oracle stays all C. */
#define shader_param_array_count oracle_shader_param_array_count
#define shader_param_int_value oracle_shader_param_int_value
#define shader_param_float_value oracle_shader_param_float_value
#define valid_int_str oracle_valid_int_str
#define os_strtod oracle_os_strtod

#include "graphics/shader-param-values.c.h"

#undef os_strtod
#undef valid_int_str
#undef shader_param_float_value
#undef shader_param_int_value
#undef shader_param_array_count

/* sizeof(long): the width the C's results past INT_MAX depend on. */
size_t oracle_shader_param_values_long_size(void);

size_t oracle_shader_param_values_long_size(void)
{
	return sizeof(long);
}
