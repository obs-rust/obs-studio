#include "libobs/build/libobs.pch.h"

bool obs_initialized(void)
{
	return obs != NULL;
}
