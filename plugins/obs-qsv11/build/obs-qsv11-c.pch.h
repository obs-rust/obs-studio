/* Precompiled header (BUILD.md) for the plugins/obs-qsv11 unity groups
 * obs-qsv11.c. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* obs-qsv11-plugin-main.c */
#include <obs-module.h>

#include "plugins/obs-qsv11/common_utils.h"

/* obs-qsv11.c */
#include <stdio.h>
#include <inttypes.h>
#include <util/dstr.h>
#include <util/darray.h>
#include <util/platform.h>
#include <util/threading.h>
#include <obs-hevc.h>
#include <obs-avc.h>

#include "plugins/obs-qsv11/QSV_Encoder.h"

/* qsv-probe interface (#1863) */
#include "plugins/obs-qsv11/qsv-probe.h"

/* qsv-host interface (#860) */
#include "plugins/obs-qsv11/qsv-host.h"
