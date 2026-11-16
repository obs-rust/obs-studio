#include "libobs/build/libobs-graphics.pch.h"

void gs_vertex2f(float x, float y)
{
	struct vec3 v3;
	vec3_set(&v3, x, y, 0.0f);
	gs_vertex3v(&v3);
}

void gs_normal3f(float x, float y, float z)
{
	struct vec3 v3;
	vec3_set(&v3, x, y, z);
	gs_normal3v(&v3);
}
