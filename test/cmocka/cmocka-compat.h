/* cmocka 1.1 support for the tests in this directory (#2197). Include it
 * after <cmocka.h>. Debian 13 and Ubuntu 24.04 ship cmocka 1.1.7, which does
 * not have some of the cmocka 2.x names. Each name here is defined only when
 * cmocka does not define it, so with cmocka 2.x this header does nothing.
 * ci/test_cmocka_compat.py lists the 2.x-only names. */

#pragma once

#ifndef assert_int_in_range
/* cmocka 1.1 has only assert_in_range(), which compares as unsigned. Use it
 * only with a minimum of 0 or more: a negative value then fails, as it must. */
#define assert_int_in_range(value, minimum, maximum) assert_in_range(value, minimum, maximum)
#endif
