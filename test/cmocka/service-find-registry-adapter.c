/* Fixture-owned registry, using the actual production field projections. */
#include "obs-internal.h"
extern struct obs_core *service_find_test_core;
#define obs service_find_test_core
#include "libobs/obs-service-find-fields.c.h"
#undef obs
