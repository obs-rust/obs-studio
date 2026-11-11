/* The Windows obs-util archive also contains filename formatting. Source-step
 * fixtures never use its video-info query; fixture-owned definitions win. */
#include <stdbool.h>
#include <stdlib.h>

struct obs_video_info;
bool obs_get_video_info(struct obs_video_info *info);

bool obs_get_video_info(struct obs_video_info *info)
{
	(void)info;
	abort();
}
