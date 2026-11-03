#!/usr/bin/env -S uv run --script
"""Run the V4L2 settings callback and capture timeout code natively (#3506).

Run with: uv run python -m unittest ci/test_v4l2_update.py
The device/library stubs check that format settings are only read while capture
is stopped. The callback and timeout statements come from production source.
"""

from __future__ import annotations

import os
import re
import shutil
import sys
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "ci"))
import host_platform as host

SOURCE = ROOT / "plugins/linux-v4l2/v4l2-input.c.h"

STUBS = r"""
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <sys/time.h>
#include <inttypes.h>
#include "libobs/util/threading-posix.h"
struct dstr { char *array; };
typedef struct { int unused; } obs_data_t;
struct v4l2_data {
    char *settings_device_id, *device_id;
    int input, pixfmt, standard, dv_timing, color_range;
    int64_t resolution, framerate, framerate_setting;
    bool auto_reset, restore_controls;
    long timeout_frames;
    obs_data_t *controls;
    pthread_mutex_t controls_mutex;
    struct dstr controls_device;
    int dev;
};
#define V4L2_DATA(ptr) struct v4l2_data *data = ptr
#define V4L2_CONTROLS_DEVICE "controls_device"
#define blog(...) ((void)0)
static bool restart, running, first_update;
static int stopped, initialized;
static int64_t requested_resolution = -1, requested_framerate = -1;
static obs_data_t settings_object;
static bool v4l2_settings_changed(struct v4l2_data *d, obs_data_t *s)
{ (void)d; (void)s; return restart; }
static void v4l2_terminate(struct v4l2_data *d)
{ (void)d; stopped += running; running = false; }
static void v4l2_update_device(struct v4l2_data *d, obs_data_t *s)
{ (void)s; assert(!running); d->settings_device_id = "camera"; }
static int64_t obs_data_get_int(obs_data_t *s, const char *key)
{
    (void)s;
    if (!strcmp(key, "timeout_frames")) return 12;
    /* Even rewriting the same negotiated format races with capture. */
    assert(!running);
    if (!strcmp(key, "resolution")) return requested_resolution;
    if (!strcmp(key, "framerate")) return requested_framerate;
    if (!strcmp(key, "input")) return 1;
    if (!strcmp(key, "pixelformat")) return 2;
    if (!strcmp(key, "standard")) return 3;
    if (!strcmp(key, "dv_timing")) return 4;
    assert(!strcmp(key, "color_range"));
    return 5;
}
static bool obs_data_get_bool(obs_data_t *s, const char *key)
{ (void)s; return !strcmp(key, "auto_reset"); }
static const char *obs_data_get_string(obs_data_t *s, const char *key)
{ (void)s; (void)key; return ""; }
static void obs_data_release(obs_data_t *s) { (void)s; }
static obs_data_t *obs_data_create(void) { return &settings_object; }
static void obs_data_apply(obs_data_t *d, obs_data_t *s) { (void)d; (void)s; }
static void dstr_copy(struct dstr *d, const char *s) { (void)d; (void)s; }
static void v4l2_update_source_flags(struct v4l2_data *d, obs_data_t *s)
{ (void)d; (void)s; }
static void v4l2_init(struct v4l2_data *d, void *unused)
{
    (void)unused;
    assert(!running);
    assert(d->input == 1 && d->pixfmt == 2 && d->standard == 3);
    assert(d->dv_timing == 4 && d->color_range == 5);
    assert(d->resolution == requested_resolution && d->framerate == requested_framerate);
    assert(first_update || stopped == 1);
    initialized++;
}
static void obs_data_set_int(obs_data_t *s, const char *k, int64_t v)
{ (void)s; (void)k; (void)v; }
static void v4l2_restore_controls(struct v4l2_data *d) { (void)d; }
static void v4l2_sync_settings(struct v4l2_data *d, obs_data_t *s) { (void)d; (void)s; }
static void v4l2_unpack_tuple(int *n, int *d, int64_t value)
{ (void)value; *n = 1; *d = 30; }
"""

CHECKS = r"""
static bool stop_reader, reader_started;
static pthread_mutex_t reader_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t reader_ready = PTHREAD_COND_INITIALIZER;
static void *read_capture_settings(void *opaque)
{
    struct v4l2_data *data = opaque;
    pthread_mutex_lock(&reader_mutex);
    reader_started = true;
    pthread_cond_signal(&reader_ready);
    pthread_mutex_unlock(&reader_mutex);
    do {
        assert(capture_reset(data));
        assert(os_atomic_load_long(&data->timeout_frames) == 12);
        assert(data->framerate == 30 && data->pixfmt == 2 && data->color_range == 5);
    } while (!os_atomic_load_bool(&stop_reader));
    return NULL;
}
int main(int argc, char **argv)
{
    assert(argc == 2);
    int mode = atoi(argv[1]);
    struct v4l2_data data = { .settings_device_id = "camera", .dev = -1,
        .input = 1, .pixfmt = 2, .standard = 3, .dv_timing = 4, .color_range = 5,
        .resolution = 1920, .framerate = 30,
        .controls_mutex = PTHREAD_MUTEX_INITIALIZER };
    if (mode == 3) {
        exercise_timeout(&data);
        return 0;
    }
    running = mode != 2;
    restart = mode == 1 || mode == 2;
    first_update = mode == 2;
    if (first_update) data.settings_device_id = NULL;
    v4l2_update(&data, &settings_object);
    assert(os_atomic_load_bool(&data.auto_reset));
    assert(os_atomic_load_long(&data.timeout_frames) == 12);
    if (mode == 4) {
        pthread_t reader;
        pthread_mutex_lock(&reader_mutex);
        assert(pthread_create(&reader, NULL, read_capture_settings, &data) == 0);
        while (!reader_started) pthread_cond_wait(&reader_ready, &reader_mutex);
        pthread_mutex_unlock(&reader_mutex);
        for (int i = 0; i < 10000; i++) v4l2_update(&data, &settings_object);
        os_atomic_store_bool(&stop_reader, true);
        assert(pthread_join(reader, NULL) == 0);
        pthread_cond_destroy(&reader_ready);
        pthread_mutex_destroy(&reader_mutex);
    }
    if (!restart) {
        assert(data.resolution == 1920 && data.framerate == 30);
        assert(stopped == 0 && initialized == 0 && running);
    } else {
        assert(initialized == 1);
        assert(stopped == !first_update);
    }
    pthread_mutex_destroy(&data.controls_mutex);
    return 0;
}
"""


class V4L2Update(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not host.is_linux():
            raise unittest.SkipTest("Linux V4L2 capture regression")
        compiler = shutil.which("cc")
        if not compiler:
            raise unittest.SkipTest("C compiler is unavailable")
        text = SOURCE.read_text()
        start = text.rindex("static void v4l2_update(void *vptr,")
        end = text.index("\n/* Record the device", start)
        update = text[start:end]
        capture = text[text.index("static bool v4l2_capture(") : text.index("static bool v4l2_reopen(")]
        setup = capture[capture.index("\t/* Get framerate") : capture.index("\n\tif (v4l2_start_capture")]
        timeout = capture[capture.index("\t\t/* Set timeout") : capture.index("\n\t\tr = select(")]
        probe = (
            "static void exercise_timeout(struct v4l2_data *data) {\n"
            "int fps_num, fps_denom; float ffps; uint64_t timeout_usec; struct timeval tv;\n"
            "os_atomic_store_long(&data->timeout_frames, 5);\n"
            + setup
            + "\n"
            + timeout
            + "\nassert(tv.tv_usec == 166666);\n"
            "os_atomic_store_long(&data->timeout_frames, 12);\n"
            + timeout
            + "\nassert(tv.tv_usec == 400000);\n}\n"
        )
        reset = re.search(r"if \((.*auto_reset.*)\)", capture)
        if reset is None:
            raise AssertionError("capture auto-reset decision not found")
        probe += "static bool capture_reset(struct v4l2_data *data) { return " + reset[1] + "; }\n"
        cls.directory = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.directory.cleanup)
        source = Path(cls.directory.name) / "probe.c"
        cls.executable = source.with_suffix("")
        source.write_text(STUBS + update + probe + CHECKS)
        sanitizer = ["-fsanitize=thread", "-g"] if os.environ.get("V4L2_UPDATE_TSAN") == "1" else []
        compiled = subprocess.run(
            [compiler, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I", str(ROOT),
             str(source), "-pthread", *sanitizer, "-o", str(cls.executable)],
            capture_output=True, text=True, timeout=30,
        )
        if compiled.returncode:
            raise AssertionError(compiled.stderr)

    def exercise(self, mode: int):
        run = subprocess.run([str(self.executable), str(mode)], capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 0, run.stderr)

    def test_running_capture_keeps_negotiated_format(self):
        self.exercise(0)

    def test_restart_stops_capture_before_changing_format(self):
        self.exercise(1)

    def test_first_update_initializes_format(self):
        self.exercise(2)

    def test_capture_reads_during_repeated_live_updates(self):
        self.exercise(4)

    def test_running_capture_observes_new_timeout(self):
        self.exercise(3)


if __name__ == "__main__":
    unittest.main()
