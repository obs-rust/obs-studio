#pragma once

/* Compiler builtins where GCC or Clang (clang-cl too) provide them, a portable
 * bit count otherwise (#851). */
#if defined(__GNUC__) || defined(__clang__)
static inline uint32_t clz32(uint32_t val)
{
	return (uint32_t)__builtin_clz(val);
}

static inline uint32_t ctz32(uint32_t val)
{
	return (uint32_t)__builtin_ctz(val);
}
#else
static inline uint32_t popcnt(uint32_t x)
{
	x -= ((x >> 1) & 0x55555555);
	x = (((x >> 2) & 0x33333333) + (x & 0x33333333));
	x = (((x >> 4) + x) & 0x0f0f0f0f);
	x += (x >> 8);
	x += (x >> 16);
	return x & 0x0000003f;
}

static inline uint32_t clz32(uint32_t x)
{
	x |= (x >> 1);
	x |= (x >> 2);
	x |= (x >> 4);
	x |= (x >> 8);
	x |= (x >> 16);
	return 32 - popcnt(x);
}

static inline uint32_t ctz32(uint32_t x)
{
	return popcnt((x & (~x + 1)) - 1);
}
#endif

static inline uint32_t min_u32(uint32_t a, uint32_t b)
{
	return (a < b) ? a : b;
}

static inline uint16_t min_u16(uint16_t a, uint16_t b)
{
	return (a < b) ? a : b;
}

static inline int32_t min_i32(int32_t a, int32_t b)
{
	return (a < b) ? a : b;
}

static inline uint8_t max_u8(uint8_t a, uint8_t b)
{
	return (a > b) ? a : b;
}
