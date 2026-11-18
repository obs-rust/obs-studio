#pragma once

#include "obs-internal.h"

/* Test-enabled libraries only; no installed header or production seam. */
EXPORT struct obs_core *obs_test_set_locale_swap_core(struct obs_core *replacement);
