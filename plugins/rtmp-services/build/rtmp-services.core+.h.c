/* Unity group (BUILD.md): the RTMP services, the custom server and the service-specific ingests. */

#include "plugins/rtmp-services/build/rtmp-services.pch.h"

#include "plugins/rtmp-services/rtmp-common.c.h"
#include "plugins/rtmp-services/rtmp-custom.c.h"
#include "plugins/rtmp-services/rtmp-services-main.c.h"
#include "plugins/rtmp-services/service-specific/amazon-ivs.c.h"
#include "plugins/rtmp-services/service-specific/dacast.c.h"
#include "plugins/rtmp-services/service-specific/nimotv.c.h"
#include "plugins/rtmp-services/service-specific/rtmps-upgrade.c.h"
#include "plugins/rtmp-services/service-specific/service-ingest.c.h"
#include "plugins/rtmp-services/service-specific/showroom.c.h"
#include "plugins/rtmp-services/service-specific/twitch.c.h"
