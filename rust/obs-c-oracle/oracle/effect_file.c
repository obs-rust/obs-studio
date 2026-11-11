/* Original C and the real private adapter with the same TLS collaborators. */
#include "graphics/graphics.h"
gs_effect_t *ef_from_file(const char *file, char **error);
#include "../../../test/cmocka/effect-file-fixture.h"

#define gs_get_context ef_context
#define os_quick_read_utf8_file ef_read
#define gs_effect_create ef_create
#define bfree ef_free
#define blog ef_blog
#include "graphics-effect-file-fields.inc"
#undef blog
#undef bfree
#undef gs_effect_create
#undef os_quick_read_utf8_file
#undef gs_get_context

void oracle_effect_file_reset(bool context, bool read_ok, bool create_ok);
void oracle_effect_file_entry(size_t index, char *path);
void *oracle_effect_file_run(const char *file, char **error);
void oracle_effect_file_args(const char *file, char **error);
const char *oracle_effect_file_events(void);
const char *oracle_effect_file_message(void);
int oracle_effect_file_level(void);
bool oracle_effect_file_args_ok(void);
bool oracle_effect_file_forwarded(void);

void oracle_effect_file_reset(bool context, bool read_ok, bool create_ok)
{
	ef_reset(context, read_ok, create_ok);
}
void oracle_effect_file_entry(size_t index, char *path)
{
	ef_effects[index].effect_path = path;
	if (index)
		ef_effects[index - 1].next = &ef_effects[index];
	else
		ef_graphics.first_effect = &ef_effects[0];
}
void oracle_effect_file_args(const char *file, char **error)
{
	ef_file = file;
	ef_error = error;
}
void *oracle_effect_file_run(const char *file, char **error)
{
	return ef_from_file(file, error);
}
const char *oracle_effect_file_events(void)
{
	return ef_events;
}
const char *oracle_effect_file_message(void)
{
	return ef_log;
}
int oracle_effect_file_level(void)
{
	return ef_level;
}
bool oracle_effect_file_args_ok(void)
{
	return ef_args_ok;
}
bool oracle_effect_file_forwarded(void)
{
	return ef_error_forwarded;
}
