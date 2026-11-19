/* COFF fallbacks must be loaded even when no platform oracle is referenced. */
#if defined(_MSC_VER)
#pragma comment(linker, "/alternatename:obs_get_video_info=oracle_obs_get_video_info")
typedef int obs_video_info_query_fallback_directive;
#else
#include <stdbool.h>
struct obs_video_info;
bool oracle_obs_get_video_info(struct obs_video_info *info);
bool obs_get_video_info(struct obs_video_info *info);
__attribute__((weak)) bool obs_get_video_info(struct obs_video_info *info)
{
	return oracle_obs_get_video_info(info);
}
#endif
