/* The xwin SDK's MSVC headers declare _mm_set_ps/_mm_setzero_ps as extern
 * functions (MSVC's compiler recognizes them as intrinsics and inlines them).
 * clang, linking for the Windows target from Linux, calls them instead. Provide
 * the three functions the shared libobs link uses so the cross link matches the
 * native build's behavior; this file is only compiled into OBS_CROSS_LINK.
 *
 * No <emmintrin.h>: on a Linux host it defines these as static inline, and it
 * is not needed for a definition whose ABI is the 16-byte vector in xmm0. */

typedef float obs_cross_m128 __attribute__((__vector_size__(16), __aligned__(16)));

obs_cross_m128 _mm_set_ps(float _A, float _B, float _C, float _D)
{
	obs_cross_m128 v = {_D, _C, _B, _A};
	return v;
}

obs_cross_m128 _mm_set_ps1(float _A)
{
	obs_cross_m128 v = {_A, _A, _A, _A};
	return v;
}

obs_cross_m128 _mm_setzero_ps(void)
{
	obs_cross_m128 v = {0.0f, 0.0f, 0.0f, 0.0f};
	return v;
}
