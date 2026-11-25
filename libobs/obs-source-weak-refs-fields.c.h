#include "libobs/build/libobs.pch.h"

/* The control stays C-owned; Rust borrows only its weak count. */
volatile long *obs_weak_source_weak_refs_field(obs_weak_source_t *weak);
volatile long *obs_weak_source_weak_refs_field(obs_weak_source_t *weak)
{
	return &weak->ref.weak_refs;
}
