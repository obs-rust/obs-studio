/* Demand-link the real logging primitives without overriding fixture services. */
#define blog source_update_properties_log_blog
#define blogva source_update_properties_log_blogva
#define bcrash source_update_properties_log_bcrash
#include "libobs/util/base-variadic.c.h"
