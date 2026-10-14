#!/usr/bin/env -S uv run --script
"""Load the production mapping helper against old and new VA runtimes (#625)."""

from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class VaapiRuntimeCompatibilityTest(unittest.TestCase):
    def test_mapping_preserves_runtime_features_and_errors(self):
        with tempfile.TemporaryDirectory(prefix="obs-vaapi-compat-") as scratch:
            root = Path(scratch)
            (root / "va").mkdir()
            (root / "va/va.h").write_text(
                "typedef void *VADisplay; typedef unsigned int VABufferID; typedef int VAStatus;\n"
                "VAStatus vaMapBuffer(VADisplay,VABufferID,void **);\n"
                "VAStatus vaMapBuffer2(VADisplay,VABufferID,void **,unsigned int);\n"
            )
            (root / "provider.c").write_text(
                '#include <stdint.h>\n#include "va/va.h"\n'
                "VAStatus vaMapBuffer(VADisplay d,VABufferID b,void **p) { *p=(void *)(uintptr_t)42; return (int)(uintptr_t)d; }\n"
                "#ifdef MODERN\n"
                "VAStatus vaMapBuffer2(VADisplay d,VABufferID b,void **p,unsigned int f) { *p=(void *)(uintptr_t)f; return (int)(uintptr_t)d; }\n"
                "#endif\n"
            )
            (root / "mapping.c").write_text(
                '#include "vaapi-runtime-compat.h"\n'
                "VAStatus map(VADisplay d,VABufferID b,void **p,unsigned int f) { return obs_va_map_buffer(d,b,p,f); }\n"
            )
            (root / "run.c").write_text(
                '#include <dlfcn.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include "va/va.h"\n'
                "int main(int argc,char **argv) { void *h=dlopen(argv[1],RTLD_NOW); "
                'if(!h) { fprintf(stderr,"%s\\n",dlerror()); return 2; } '
                'VAStatus (*map)(VADisplay,VABufferID,void **,unsigned int)=dlsym(h,"map"); '
                "if(!map) return 3; void *p=0; int status=atoi(argv[2]); "
                "int result=map((void *)(uintptr_t)status,0,&p,5); "
                'printf("%d %lu\\n",result,(unsigned long)(uintptr_t)p); dlclose(h); }\n'
            )
            subprocess.run(
                [
                    "cc",
                    "-I",
                    str(root),
                    str(root / "run.c"),
                    "-ldl",
                    "-o",
                    str(root / "run"),
                ],
                check=True,
                capture_output=True,
            )
            for modern in (False, True):
                directory = root / ("modern" if modern else "legacy")
                directory.mkdir()
                command = ["cc", "-fPIC", "-shared", "-I", str(root)]
                if modern:
                    command.append("-DMODERN=1")
                subprocess.run(
                    command
                    + [
                        str(root / "provider.c"),
                        "-Wl,-soname,libva.so.2",
                        "-o",
                        str(directory / "libva.so.2"),
                    ],
                    check=True,
                    capture_output=True,
                )
                subprocess.run(
                    [
                        "cc",
                        "-fPIC",
                        "-shared",
                        "-I",
                        str(root),
                        "-I",
                        str(ROOT / "build-aux/appimage"),
                        str(root / "mapping.c"),
                        "-L",
                        str(directory),
                        "-Wl,--no-as-needed",
                        "-l:libva.so.2",
                        "-Wl,-rpath,$ORIGIN",
                        "-ldl",
                        "-o",
                        str(directory / "mapping.so"),
                    ],
                    check=True,
                    capture_output=True,
                )
                for error in (0, 7):
                    with self.subTest(modern=modern, error=error):
                        completed = subprocess.run(
                            [
                                str(root / "run"),
                                str(directory / "mapping.so"),
                                str(error),
                            ],
                            capture_output=True,
                            text=True,
                        )
                        self.assertEqual(completed.returncode, 0, completed.stderr)
                        self.assertEqual(
                            completed.stdout.strip(), f"{error} {5 if modern else 42}"
                        )
