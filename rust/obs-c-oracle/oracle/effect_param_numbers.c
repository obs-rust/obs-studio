/* Test-only oracle: the original libobs/graphics/effect-param-numbers.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. valid_int_str is the C lexer.c, already renamed in
 * oracle/lexer.c, and strtol the C library's, so the oracle stays all C. */
#define obs_effect_param_int_value oracle_obs_effect_param_int_value
#define obs_effect_param_array_count oracle_obs_effect_param_array_count
#define valid_int_str oracle_valid_int_str

#include "graphics/effect-param-numbers.c.h"

#undef valid_int_str
#undef obs_effect_param_array_count
#undef obs_effect_param_int_value

/* sizeof(long): the width the C's results past INT_MAX depend on. */
size_t oracle_effect_param_numbers_long_size(void);

size_t oracle_effect_param_numbers_long_size(void)
{
	return sizeof(long);
}
