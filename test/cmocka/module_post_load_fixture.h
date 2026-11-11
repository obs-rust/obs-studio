#pragma once

#include "obs-internal.h"

/* Test-enabled libraries only; no installed/public header or production seam. */
EXPORT struct obs_core *obs_test_module_post_load_swap_core(struct obs_core *replacement);
