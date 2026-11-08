/* Original release bodies over the real private C layout. */
#define obs_weak_service_strong_release oracle_obs_weak_service_strong_release
#define obs_weak_service_weak_addref oracle_obs_weak_service_weak_addref
#define obs_weak_service_weak_release oracle_obs_weak_service_weak_release
#define obs_weak_service_free oracle_obs_weak_service_free
#include "obs-service-weak-release.c.h"
#undef obs_weak_service_free
#undef obs_weak_service_weak_release
#undef obs_weak_service_weak_addref
#undef obs_weak_service_strong_release
