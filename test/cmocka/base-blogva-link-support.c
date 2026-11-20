/* Requesting the real va_list forwarder must not also select blog or bcrash. */
#define base_default_log_handler obs_link_support_blogva_default_handler
#define base_log_literal obs_link_support_blogva_literal
#define blog obs_link_support_blogva_blog
#define bcrash obs_link_support_blogva_bcrash
#include "libobs/util/base-variadic.c.h"
