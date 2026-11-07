/* Pure scene-rule tests link the production Rust archive without starting OBS.
 * The same archive member now contains public getters that reference blog.
 * Their logging is covered by actual-library and oracle tests. These rule tests
 * must never reach that dependency: fail if they unexpectedly call it. */
#include <stdlib.h>
#include <util/base.h>

void blog(int level, const char *format, ...)
{
	(void)level;
	(void)format;
	abort();
}

/* Private mutex adapters are deliberately hidden in libobs. Pure scene-rule
 * consumers must supply them too, even when linking the shared library. */
struct obs_scene;
struct obs_scene_item;
struct obs_scene *obs_sceneitem_fields_lock(const struct obs_scene_item *item);
void obs_sceneitem_fields_unlock(struct obs_scene *scene);
struct obs_scene *obs_sceneitem_fields_lock(const struct obs_scene_item *item)
{
	(void)item;
	abort();
}
void obs_sceneitem_fields_unlock(struct obs_scene *scene)
{
	(void)scene;
	abort();
}
