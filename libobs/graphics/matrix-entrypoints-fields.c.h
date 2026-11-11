#include "libobs/build/libobs-graphics.pch.h"

/* Private layout access only; Rust owns the entry-point orchestration. */
graphics_t *obs_graphics_matrix_context(const char *function);
struct darray *obs_graphics_matrix_stack(graphics_t *graphics);
size_t *obs_graphics_matrix_current(graphics_t *graphics);
void obs_graphics_matrix_log_pop_last(void);

graphics_t *obs_graphics_matrix_context(const char *function)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
	return graphics;
}
struct darray *obs_graphics_matrix_stack(graphics_t *graphics)
{
	return &graphics->matrix_stack.da;
}
size_t *obs_graphics_matrix_current(graphics_t *graphics)
{
	return &graphics->cur_matrix;
}
void obs_graphics_matrix_log_pop_last(void)
{
	/* gs_matrix_pop's error when the stack held only the current matrix. */
	blog(LOG_ERROR, "Tried to pop last matrix on stack");
}
