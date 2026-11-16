#include "libobs/build/libobs-graphics.pch.h"

/* The matrix stack is in graphics-state-stacks.c. */

void gs_matrix_push(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_matrix_push"))
		return;

	gs_matrix_stack_push(&graphics->matrix_stack.da, &graphics->cur_matrix);
}

void gs_matrix_pop(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_matrix_pop"))
		return;

	if (!gs_matrix_stack_pop(&graphics->matrix_stack.da, &graphics->cur_matrix))
		blog(LOG_ERROR, "Tried to pop last matrix on stack");
}

void gs_matrix_identity(void)
{
	if (!gs_valid("gs_matrix_identity"))
		return;

	gs_matrix_stack_identity(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix);
}

void gs_matrix_transpose(void)
{
	if (!gs_valid("gs_matrix_transpose"))
		return;

	gs_matrix_stack_transpose(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix);
}

void gs_matrix_set(const struct matrix4 *matrix)
{
	if (!gs_valid("gs_matrix_set"))
		return;

	gs_matrix_stack_set(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, matrix);
}

void gs_matrix_get(struct matrix4 *dst)
{
	if (!gs_valid("gs_matrix_get"))
		return;

	gs_matrix_stack_get(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, dst);
}

void gs_matrix_mul(const struct matrix4 *matrix)
{
	if (!gs_valid("gs_matrix_mul"))
		return;

	gs_matrix_stack_mul(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, matrix);
}

void gs_matrix_rotquat(const struct quat *rot)
{
	if (!gs_valid("gs_matrix_rotquat"))
		return;

	gs_matrix_stack_rotquat(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, rot);
}

void gs_matrix_rotaa(const struct axisang *rot)
{
	if (!gs_valid("gs_matrix_rotaa"))
		return;

	gs_matrix_stack_rotaa(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, rot);
}

void gs_matrix_translate(const struct vec3 *pos)
{
	if (!gs_valid("gs_matrix_translate"))
		return;

	gs_matrix_stack_translate(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, pos);
}

void gs_matrix_scale(const struct vec3 *scale)
{
	if (!gs_valid("gs_matrix_scale"))
		return;

	gs_matrix_stack_scale(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, scale);
}

void gs_matrix_rotaa4f(float x, float y, float z, float angle)
{
	struct axisang aa;

	if (!gs_valid("gs_matrix_rotaa4f"))
		return;

	axisang_set(&aa, x, y, z, angle);
	gs_matrix_stack_rotaa(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, &aa);
}

void gs_matrix_translate3f(float x, float y, float z)
{
	struct vec3 p;

	if (!gs_valid("gs_matrix_translate3f"))
		return;

	vec3_set(&p, x, y, z);
	gs_matrix_stack_translate(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, &p);
}

void gs_matrix_scale3f(float x, float y, float z)
{
	struct vec3 p;

	if (!gs_valid("gs_matrix_scale3f"))
		return;

	vec3_set(&p, x, y, z);
	gs_matrix_stack_scale(&thread_graphics->matrix_stack.da, thread_graphics->cur_matrix, &p);
}
