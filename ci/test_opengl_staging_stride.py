#!/usr/bin/env -S uv run --script
"""Exercise the actual OpenGL staging allocation/map functions without a GPU (#4126)."""

import ctypes
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from test_settings_platform_facade import function

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipIf(sys.platform == "win32", "fixture uses a Unix shared library")
class OpenGLStagingStrideTest(unittest.TestCase):
    def test_mapped_rows_match_the_allocated_pack_pitch(self):
        source = (ROOT / "libobs-opengl/gl-stagesurf.c.h").read_text()
        # Compile production function bodies verbatim. Only the GL calls are
        # substituted: the buffer allocation records its size and mapping
        # returns storage containing distinct row payloads and padding.
        code = r'''
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#define GL_PIXEL_PACK_BUFFER 1
#define GL_DYNAMIC_READ 2
#define GL_READ_ONLY 3
#define LOG_ERROR 4
typedef ptrdiff_t GLsizeiptr;
typedef struct gs_stage_surface {
    uint32_t width, height, bytes_per_pixel, pack_buffer;
} gs_stagesurf_t;
static uint8_t *storage;
static size_t allocation;
static bool gl_gen_buffers(int count, uint32_t *buffer)
{ (void)count; *buffer = 1; return true; }
static bool gl_bind_buffer(int target, uint32_t buffer)
{ (void)target; (void)buffer; return true; }
static void glBufferData(int target, GLsizeiptr size, const void *data, int usage)
{
    (void)target; (void)data; (void)usage;
    free(storage);
    allocation = (size_t)size;
    storage = calloc(allocation, 1);
}
static bool gl_success(const char *operation)
{ (void)operation; return storage != NULL; }
static void *glMapBuffer(int target, int access)
{ (void)target; (void)access; return storage; }
static void blog(int level, const char *message)
{ (void)level; (void)message; }
''' + function(source, "static bool create_pixel_pack_buffer(") + "\n" + function(
            source, "bool gs_stagesurface_map(") + r'''
uint32_t mapped_stride(uint32_t width, uint32_t bytes_per_pixel, uint32_t height,
                       uint8_t **data, size_t *size)
{
    gs_stagesurf_t surface = {width, height, bytes_per_pixel, 0};
    uint32_t stride = 0;
    if (!create_pixel_pack_buffer(&surface) ||
        !gs_stagesurface_map(&surface, data, &stride))
        return 0;
    *size = allocation;
    return stride;
}
void release_storage(void) { free(storage); storage = NULL; }
'''
        compilers = [compiler for name in ("gcc", "clang") if (compiler := shutil.which(name))]
        if not compilers:
            self.skipTest("C compiler unavailable; native build remains required")
        for compiler in compilers:
            with self.subTest(compiler=compiler), tempfile.TemporaryDirectory(prefix="obs-staging-stride-") as tmp:
                path = Path(tmp)
                (path / "probe.c").write_text(code)
                result = subprocess.run(
                    [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                     str(path / "probe.c"), "-o", str(path / "probe.so")],
                    capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                library = ctypes.CDLL(str(path / "probe.so"))
                library.mapped_stride.argtypes = [ctypes.c_uint32] * 3 + [
                    ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_size_t)]
                library.mapped_stride.restype = ctypes.c_uint32
                library.release_storage.restype = None
                try:
                    for width in (1, 2, 3, 4, 16, 34, 36, 682, 684):
                        for bpp in (1, 2, 4, 8, 16):
                            with self.subTest(width=width, bytes_per_pixel=bpp):
                                data = ctypes.POINTER(ctypes.c_uint8)()
                                size = ctypes.c_size_t()
                                height = 4
                                pitch = library.mapped_stride(width, bpp, height, ctypes.byref(data), ctypes.byref(size))
                                expected = ((width * bpp + 3) // 4) * 4
                                self.assertEqual(size.value, expected * height)
                                self.assertEqual(pitch, expected)
                                # Model four-byte GL pack alignment independently
                                # of the reported pitch, then consume mapped rows.
                                for row in range(height):
                                    for column in range(expected):
                                        data[row * expected + column] = row + 1 if column < width * bpp else 255
                                for row in range(height):
                                    actual = bytes(data[row * pitch:row * pitch + width * bpp])
                                    self.assertEqual(actual, bytes([row + 1]) * (width * bpp))
                finally:
                    library.release_storage()


if __name__ == "__main__":
    unittest.main()
