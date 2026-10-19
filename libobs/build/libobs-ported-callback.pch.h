/* Precompiled header (BUILD.md) for the libobs unity group libobs.ported-callback.
 * callback/calldata.c and callback/decl.c alone, without the core, so the C oracle and C tests can compile them. C.
 * The implementation files include this header and nothing else: the includes below are theirs. */

#pragma once

/* callback/calldata.c */
#include <string.h>

#include "util/bmem.h"
#include "util/base.h"
#include "callback/calldata.h"

/* callback/decl.c */
#include "util/cf-parser.h"
#include "callback/decl.h"
