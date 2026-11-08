/* These standalone step tests never query a module registry. */
#include <stdbool.h>
#include <stdlib.h>

void *const *obs_module_name_lookup_head_field(bool disabled);
void *const *obs_module_name_lookup_head_field(bool disabled)
{
	(void)disabled;
	abort();
}
char *const *obs_module_name_lookup_name_field(const void *module);
char *const *obs_module_name_lookup_name_field(const void *module)
{
	(void)module;
	abort();
}
void *const *obs_module_name_lookup_next_field(const void *module);
void *const *obs_module_name_lookup_next_field(const void *module)
{
	(void)module;
	abort();
}
