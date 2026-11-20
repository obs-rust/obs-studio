/* Retain the production graphics runtime only for consumers that request it.
 * These files share the same private context and imports in the native group. */
#define gs_preprocessor_name obs_link_support_preprocessor_name
#include "libobs/graphics/graphics-imports.c.h"
#include "libobs/graphics/graphics.c.h"
#include "libobs/graphics/viewport-entrypoints.c.h"
