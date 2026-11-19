/* Keep the real crash provider independent of fixture-owned public loggers. */
#define base_default_log_handler obs_link_support_crash_default_handler
#define base_log_literal obs_link_support_crash_literal
#define blog obs_link_support_crash_blog
#define blogva obs_link_support_crash_blogva
#include "libobs/util/base-variadic.c.h"
