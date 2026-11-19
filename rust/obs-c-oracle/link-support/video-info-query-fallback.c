/* This directive must be loaded even when no platform oracle is referenced.
 * /alternatename in an unselected archive member cannot resolve a new caller. */
#pragma comment(linker, "/alternatename:obs_get_video_info=oracle_obs_get_video_info")
typedef int obs_video_info_query_fallback_directive;
