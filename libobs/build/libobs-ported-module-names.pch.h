/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-names. obs-module-names.c needs only the dstr, bmem
 * and platform (os_file_exists) helpers and the declarations of its entry
 * points and of obs_module_paths_data_dir() (obs-internal.h) and of
 * struct obs_module_info2 (obs.h), so the C oracle and the C tests can
 * compile it on their own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "obs-internal.h"
