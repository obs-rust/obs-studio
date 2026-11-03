#!/usr/bin/env -S uv run --script
"""Execute the real script_load callback block with failed conversions (#3446).

Run with: uv run python -m unittest ci/test_python_script_load.py
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "shared/obs-scripting/obs-scripting-python.c.h"

STUBS = r"""
#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
typedef struct { int identity; } PyObject;
static PyObject callback, settings, none, arguments, result;
static int mode, calls, builds, errors, clears;
static int settings_refs, none_refs, args_refs, result_refs;
struct script { void *module; struct { void *settings; } base; };
static PyObject *PyObject_GetAttrString(void *module, const char *name)
{
    (void)module; (void)name;
    return mode == 3 ? NULL : &callback;
}
static bool convert(void *input, bool ownership, PyObject **output)
{
    (void)input; (void)ownership;
    if (mode == 1) return false; /* Missing SWIG type: output is untouched. */
    *output = mode == 2 ? &none : &settings;
    return mode != 2;
}
#define libobs_to_py(type, input, ownership, output) convert(input, ownership, output)
static PyObject *Py_BuildValue(const char *format, PyObject *object)
{
    (void)format;
    assert(object == &settings);
    builds++;
    return &arguments;
}
static PyObject *PyObject_CallObject(PyObject *function, PyObject *args)
{
    assert(function == &callback && args == &arguments);
    calls++;
    return &result;
}
static void py_error(void) { errors++; }
static void PyErr_Clear(void) { clears++; }
static void Py_XDECREF(PyObject *object)
{
    if (object == &settings) settings_refs++;
    else if (object == &none) none_refs++;
    else if (object == &arguments) args_refs++;
    else if (object == &result) result_refs++;
    else assert(object == NULL);
}
static void exercise(struct script *data)
{
    PyObject *py_load;
"""

CHECKS = r"""
}
int main(int argc, char **argv)
{
    assert(argc == 2);
    mode = atoi(argv[1]);
    struct script data = {0};
    exercise(&data);
    assert(calls == (mode == 0));
    assert(builds == calls && errors == calls);
    assert(settings_refs == (mode == 0));
    assert(none_refs == (mode == 2));
    assert(args_refs == calls && result_refs == calls);
    assert(clears == (mode == 3));
    return 0;
}
"""


class ScriptLoadConversion(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which("cc")
        if not compiler:
            raise unittest.SkipTest("C compiler is unavailable")
        text = SOURCE.read_text()
        start = text.index('\tpy_load = PyObject_GetAttrString(py_module, "script_load");')
        end = text.index("\n\tif (data->module)", start)
        block = text[start:end].replace("py_module", "data->module")
        cls.directory = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.directory.cleanup)
        source = Path(cls.directory.name) / "probe.c"
        cls.executable = source.with_suffix("")
        source.write_text(STUBS + block + CHECKS)
        compiled = subprocess.run(
            [compiler, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(source), "-o", str(cls.executable)],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if compiled.returncode:
            raise AssertionError(compiled.stderr)

    def exercise(self, mode: int):
        run = subprocess.run([str(self.executable), str(mode)], capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 0, run.stderr)

    def test_success_calls_script_load_and_releases_objects(self):
        self.exercise(0)

    def test_missing_swig_type_skips_callback(self):
        self.exercise(1)

    def test_failed_conversion_releases_returned_none(self):
        self.exercise(2)

    def test_missing_script_load_clears_attribute_error(self):
        self.exercise(3)


if __name__ == "__main__":
    unittest.main()
