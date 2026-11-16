#include "libobs/build/libobs-graphics.pch.h"

void gs_vertex3f(float x, float y, float z)
{
	struct vec3 v3;
	vec3_set(&v3, x, y, z);
	gs_vertex3v(&v3);
}

void gs_texcoord(float x, float y, int unit)
{
	struct vec2 v2;
	vec2_set(&v2, x, y);
	gs_texcoord2v(&v2, unit);
}

void gs_vertex2v(const struct vec2 *v)
{
	struct vec3 v3;
	vec3_set(&v3, v->x, v->y, 0.0f);
	gs_vertex3v(&v3);
}
