/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-paths. obs-module-paths.c needs only the dstr, bmem
 * and platform (os_file_exists, os_glob) helpers and the declarations of its
 * entry points, of obs_module_add_extension_t (obs-internal.h) and of
 * struct obs_module_info2 (obs.h), so the C oracle and the C tests can
 * compile it on their own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "obs-internal.h"
