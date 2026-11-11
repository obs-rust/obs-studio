/* Original wrapper, unmodified, delegates to the original view oracle.
 * Reuse oracle/view.c's real-layout view accessors and traced source doubles.
 * The caller holds that oracle's shared test lock. No graphics or OBS core. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

void oracle_obs_view_render(obs_view_t *view);
void oracle_obs_canvas_render(obs_canvas_t *canvas);
#define obs_view_render oracle_obs_view_render
#define obs_canvas_render oracle_obs_canvas_render
#include "obs-canvas-render.c.h"
#undef obs_canvas_render
#undef obs_view_render

/* Matches the already established oracle_view_source dependency fixture. */
struct oracle_view_source {
	const char *name;
	bool removed;
	bool dying;
};
struct canvas_render_fixture {
	struct obs_canvas canvas[2];
	struct oracle_view_source sources[3];
};

void *oracle_canvas_render_new(void);
void *oracle_canvas_render_new(void)
{
	return calloc(1, sizeof(struct canvas_render_fixture));
}
void oracle_canvas_render_free(void *data);
void oracle_canvas_render_free(void *data)
{
	free(data);
}
void oracle_canvas_render_reset(void *data, const unsigned char *channels, unsigned removed, uint32_t flags);
void oracle_canvas_render_reset(void *data, const unsigned char *channels, unsigned removed, uint32_t flags)
{
	struct canvas_render_fixture *f = data;
	memset(f, 0, sizeof(*f));
	const char *names[] = {"a", "b", "c"};
	for (size_t i = 0; i < 3; i++) {
		f->sources[i].name = names[i];
		f->sources[i].removed = (removed & (1u << i)) != 0;
	}
	for (size_t c = 0; c < 2; c++) {
		f->canvas[c].flags = flags;
		f->canvas[c].view.type = AUX_VIEW;
		for (size_t i = 0; i < MAX_CHANNELS; i++) {
			unsigned id = channels[c * MAX_CHANNELS + i];
			if (id > 0 && id <= 3)
				f->canvas[c].view.channels[i] = (obs_source_t *)&f->sources[id - 1];
		}
	}
}
obs_canvas_t *oracle_canvas_render_canvas(void *data, unsigned selected);
obs_canvas_t *oracle_canvas_render_canvas(void *data, unsigned selected)
{
	struct canvas_render_fixture *f = data;
	return &f->canvas[selected];
}
obs_view_t *oracle_canvas_render_view(void *data, unsigned selected);
obs_view_t *oracle_canvas_render_view(void *data, unsigned selected)
{
	return &oracle_canvas_render_canvas(data, selected)->view;
}
/* Side effects of the render callback must survive; a later channel is live
 * in this same pass. The view/source dependency owns all render semantics. */
void oracle_canvas_render_effect(void *data, unsigned selected, bool change);
void oracle_canvas_render_effect(void *data, unsigned selected, bool change)
{
	struct canvas_render_fixture *f = data;
	f->canvas[selected].flags++;
	if (change)
		f->canvas[selected].view.channels[MAX_CHANNELS - 1] = (obs_source_t *)&f->sources[1];
}
/* Exact pointers and all fixture bytes after C and Rust on the SAME fixture.
 * Mutexes are only traced by oracle/view.c, never passed to pthread. */
size_t oracle_canvas_render_size(void);
size_t oracle_canvas_render_size(void)
{
	return sizeof(struct canvas_render_fixture);
}
void oracle_canvas_render_snapshot(void *data, unsigned char *bytes);
void oracle_canvas_render_snapshot(void *data, unsigned char *bytes)
{
	memcpy(bytes, data, sizeof(struct canvas_render_fixture));
}
