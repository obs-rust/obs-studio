#pragma once

#include "obs.h"

/* Private seam: preserves the existing signal builder and synchronous dispatch. */
void obs_source_push_to_enable_signal_changed(obs_source_t *source, const char *signal, bool enabled);
