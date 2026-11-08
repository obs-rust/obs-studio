/* The extracted original C, renamed; opaque fixtures use the real private header. */
#define obs_get_module_file_name oracle_obs_get_module_file_name
#define obs_get_module_name oracle_obs_get_module_name
#define obs_get_module_author oracle_obs_get_module_author
#define obs_get_module_description oracle_obs_get_module_description
#define obs_get_module_binary_path oracle_obs_get_module_binary_path
#define obs_get_module_data_path oracle_obs_get_module_data_path
#define obs_get_module_id oracle_obs_get_module_id
#define obs_get_module_version oracle_obs_get_module_version
#define obs_get_module_lib oracle_obs_get_module_lib
#include "libobs/obs-module-metadata.c.h"
#undef obs_get_module_file_name
#undef obs_get_module_name
#undef obs_get_module_author
#undef obs_get_module_description
#undef obs_get_module_binary_path
#undef obs_get_module_data_path
#undef obs_get_module_id
#undef obs_get_module_version
#undef obs_get_module_lib
#include "libobs/obs-module-metadata-adapter.c.h"
#include <stdlib.h>

struct metadata_fixture {
	struct obs_module module;
	struct obs_module_metadata metadata;
};
/* Only the single metadata parity test uses these callbacks. */
static unsigned metadata_calls;
static const char *metadata_result;
static const char *metadata_callback(void)
{
	++metadata_calls;
	return metadata_result;
}
void *oracle_module_metadata_new(unsigned flags, char *text, char *empty, char *other);
void *oracle_module_metadata_new(unsigned flags, char *text, char *empty, char *other)
{
	struct metadata_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		abort();
	f->module.metadata = flags & 2u ? &f->metadata : NULL;
	f->metadata.display_name = flags & 4u ? empty : NULL;
	f->module.name = flags & 8u ? metadata_callback : NULL;
	f->module.author = flags & 16u ? metadata_callback : NULL;
	f->module.description = flags & 32u ? metadata_callback : NULL;
	f->module.file = flags & 64u ? text : NULL;
	f->module.bin_path = flags & 64u ? other : NULL;
	f->module.data_path = flags & 64u ? empty : NULL;
	f->module.module = flags & 64u ? f : NULL;
	f->metadata.id = flags & 128u ? other : NULL;
	f->metadata.version = flags & 256u ? text : NULL;
	f->metadata.description = other; /* deliberately ignored by the getter */
	metadata_result = flags & 512u ? NULL : text;
	return f;
}
void oracle_module_metadata_free(void *p);
void oracle_module_metadata_free(void *p)
{
	free(p);
}
unsigned oracle_module_metadata_reset_calls(void);
unsigned oracle_module_metadata_reset_calls(void)
{
	unsigned calls = metadata_calls;
	metadata_calls = 0;
	return calls;
}
bool oracle_module_metadata_field_addresses(void *p);
bool oracle_module_metadata_field_addresses(void *p)
{
	struct metadata_fixture *f = p;
	struct obs_module *m = &f->module;
	return obs_module_metadata_file_field(m) == &m->file &&
	       obs_module_metadata_binary_path_field(m) == &m->bin_path &&
	       obs_module_metadata_data_path_field(m) == &m->data_path &&
	       obs_module_metadata_lib_field(m) == &m->module && obs_module_metadata_name_field(m) == &m->name &&
	       obs_module_metadata_author_field(m) == &m->author &&
	       obs_module_metadata_description_field(m) == &m->description &&
	       obs_module_metadata_id_field(m) == (m->metadata ? &m->metadata->id : NULL) &&
	       obs_module_metadata_version_field(m) == (m->metadata ? &m->metadata->version : NULL) &&
	       obs_module_metadata_display_field(m) == (m->metadata ? &m->metadata->display_name : NULL) &&
	       obs_module_metadata_file_field(NULL) == NULL && obs_module_metadata_binary_path_field(NULL) == NULL &&
	       obs_module_metadata_data_path_field(NULL) == NULL && obs_module_metadata_lib_field(NULL) == NULL &&
	       obs_module_metadata_name_field(NULL) == NULL && obs_module_metadata_author_field(NULL) == NULL &&
	       obs_module_metadata_description_field(NULL) == NULL && obs_module_metadata_id_field(NULL) == NULL &&
	       obs_module_metadata_version_field(NULL) == NULL && obs_module_metadata_display_field(NULL) == NULL;
}
