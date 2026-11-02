/* Precompiled header (BUILD.md) for the plugins/rtmp-services unity groups
 * rtmp-services.core. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* rtmp-common.c */
#include <util/platform.h>
#include <util/dstr.h>
#include <obs-module.h>
#include <jansson.h>
#include <obs-config.h>

#include "plugins/rtmp-services/rtmp-format-ver.h"
#include "plugins/rtmp-services/service-specific/twitch.h"
#include "plugins/rtmp-services/service-specific/nimotv.h"
#include "plugins/rtmp-services/service-specific/showroom.h"
#include "plugins/rtmp-services/service-specific/dacast.h"
#include "plugins/rtmp-services/service-specific/amazon-ivs.h"
#include "plugins/rtmp-services/service-specific/rtmps-upgrade.h"

/* rtmp-services-main.c */
#include <util/text-lookup.h>
#include <util/threading.h>
#include <file-updater/file-updater.h>

/* service-specific/amazon-ivs.c */
#include "plugins/rtmp-services/service-specific/service-ingest.h"

/* service-specific/nimotv.c */
#include <util/curl/curl-helper.h>
#include <stdlib.h>
#include <string.h>
#include "util/base.h"
#include <time.h>

/* service-specific/showroom.c */
#include <util/darray.h>
