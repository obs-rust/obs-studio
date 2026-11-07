/* Original C cleanup, renamed; layout comes from the real public header. */
#define obs_module_failure_info_free oracle_obs_module_failure_info_free
#include "libobs/obs-module-failure-info.c.h"
#undef obs_module_failure_info_free

size_t oracle_module_failure_info_size(void);
size_t oracle_module_failure_info_align(void);
size_t oracle_module_failure_info_table_offset(void);
size_t oracle_module_failure_info_count_offset(void);

size_t oracle_module_failure_info_size(void)
{
	return sizeof(struct obs_module_failure_info);
}

size_t oracle_module_failure_info_align(void)
{
	return _Alignof(struct obs_module_failure_info);
}

size_t oracle_module_failure_info_table_offset(void)
{
	return offsetof(struct obs_module_failure_info, failed_modules);
}

size_t oracle_module_failure_info_count_offset(void)
{
	return offsetof(struct obs_module_failure_info, count);
}
