#!/usr/bin/env -S uv run --script
"""Exercise the native encoder fixture's first-packet publication (#3504).

Run with: uv run python -m unittest ci/test_encoder_result_publication.py
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "test/cmocka/test_encoder_join_keyframe.c"

PRELUDE = r"""
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <pthread.h>
#include "util/threading.h"
typedef void obs_output_t;
typedef void obs_data_t;
struct encoder_packet { long long pts; bool keyframe; };
static void *bzalloc(size_t size) { return calloc(1, size); }
static void bfree(void *data) { free(data); }
static void *obs_obj_get_data(void *output) { return output; }
static void os_sleep_ms(int ms) { (void)ms; }
static pthread_mutex_t pause_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t pause_cond = PTHREAD_COND_INITIALIZER;
static bool entered, proceed;
static uint64_t os_gettime_ns(void)
{
    pthread_mutex_lock(&pause_lock);
    entered = true;
    pthread_cond_signal(&pause_cond);
    while (!proceed)
        pthread_cond_wait(&pause_cond, &pause_lock);
    pthread_mutex_unlock(&pause_lock);
    return 200000000;
}
"""

CHECKS = r"""
static void *deliver(void *data)
{
    struct encoder_packet packet = {.pts = 17, .keyframe = true};
    out_packet(data, &packet);
    return NULL;
}
int main(void)
{
    struct out *o = out_create(NULL, NULL);
    assert(o);
    o->started_ns = 100000000;
    pthread_t thread;
    assert(pthread_create(&thread, NULL, deliver, o) == 0);
    pthread_mutex_lock(&pause_lock);
    while (!entered)
        pthread_cond_wait(&pause_cond, &pause_lock);
    pthread_mutex_unlock(&pause_lock);

    /* The callback is in flight, but has not written its timestamp yet. */
    bool premature = wait_packets(o, 1, 0);
    pthread_mutex_lock(&pause_lock);
    proceed = true;
    pthread_cond_signal(&pause_cond);
    pthread_mutex_unlock(&pause_lock);
    assert(pthread_join(thread, NULL) == 0);
    assert(!premature);
    assert(wait_packets(o, 1, 0));
    assert(first_packet_ms(o) == 100.0);
    assert(o->first_pts == 17);
    assert(os_atomic_load_long(&o->first_is_keyframe) == 1);

    struct encoder_packet later = {.pts = 23, .keyframe = false};
    out_packet(o, &later);
    later.pts = 19;
    out_packet(o, &later);
    out_packet(o, NULL);
    assert(os_atomic_load_long(&o->packets) == 3);
    assert(o->last_pts == 23);
    assert(o->first_pts == 17);
    assert(first_packet_ms(o) == 100.0);
    assert(os_atomic_load_long(&o->first_is_keyframe) == 1);
    out_destroy(o);
    return 0;
}
"""


def definition(text: str, signature: str) -> str:
    start = text.index(signature)
    opening = text.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    if text[end : end + 1] == ";":
        end += 1
    return text[start:end] + "\n"


class EncoderResultPublication(unittest.TestCase):
    def test_waiter_only_sees_completed_first_packet(self):
        compiler = shutil.which("cc")
        if not compiler:
            self.skipTest("C compiler is unavailable")
        text = SOURCE.read_text()
        native = PRELUDE + "\n".join(
            definition(text, signature)
            for signature in (
                "struct out {",
                "static void *out_create(",
                "static void out_destroy(",
                "static void out_packet(",
                "static bool wait_packets(",
                "static double first_packet_ms(",
            )
        ) + CHECKS
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "probe.c"
            executable = source.with_suffix("")
            source.write_text(native)
            compiled = subprocess.run(
                [compiler, "-std=c11", "-D_POSIX_C_SOURCE=200809L", "-O2", "-Wall", "-Wextra", "-Werror", "-pthread",
                 "-I", str(ROOT / "libobs"), str(source), "-o", str(executable)],
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            run = subprocess.run([str(executable)], capture_output=True, text=True, timeout=5)
            self.assertEqual(run.returncode, 0, run.stderr)


if __name__ == "__main__":
    unittest.main()
