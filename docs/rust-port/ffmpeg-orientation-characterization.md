# FFmpeg orientation metadata selection characterization

Prerequisite #4431; Rust follow-up #4432. The unit is
`ffmpeg_image_get_orientation` in `libobs/graphics/graphics-ffmpeg.c.h`.

The existing, unchanged `test/cmocka/test_image_exif_orientation.c` generates
32 by 16 JPEG fixtures with four distinct color quadrants and EXIF orientation
values 1–8, plus a JPEG with no EXIF block. It calls the public
`gs_create_texture_file_data()` API and checks the resulting dimensions and
four corner colors. Thus it exercises metadata selection through real decoding,
without exposing the static function or requiring a graphics backend.

On 2026-11-09, before extraction at b431a40b6da74ed753af4dc4faa0f3637ba2b5a5,
`ctest --verbose -R '^test_image_exif_orientation$'` passed with each of
`libobs-c-reference` and hybrid `libobs` selected through `LD_LIBRARY_PATH`.
The test also runs PNG and WebP cases; PNG EXIF orientations 2–8 skip on this
FFmpeg version. No existing C tests or test graph registrations are changed.

The Linux Tier 2 container uses FFmpeg 8.0 (libavcodec 62.11.100, libavutil 60.8.100). This selects
`!FFMPEG_HAS_EXIF_API` (libavcodec before 62.13.100). The JPEG decoder reports
the case-sensitive `Orientation` frame metadata for values 1–8; the no-EXIF
case exercises absence of metadata and display matrix. These public image
fixtures do not independently test precedence when metadata and a matrix
coexist, lowercase metadata keys, or an undersized side-data allocation.
They do not exercise the FFmpeg 8.1 `av_exif_matrix_to_orientation` branch.
Those limits must remain explicit when the Rust follow-up adds safe-core
and differential coverage.

After the verbatim extraction into `graphics-ffmpeg-orientation.c.h`, the same
command passed against both libraries again: 20 cases passed and seven PNG
cases skipped in each run. Every JPEG orientation case passed before and after.
The exported symbol lists of hybrid and C-reference libraries were identical
before and after. The extracted block includes the existing small static
`display_matrix_to_orientation` adapter, so alphabetical unity inclusion needs
no forward declarations or changes to its Rust math dependency. Decoder
selection, initialization, cleanup, and decode orchestration remain untouched.

Reproduction: configure the normal Linux Tier 2 tree with
`OBS_BUILD_C_REFERENCE=ON` and `OBS_LIBOBS_BACKEND=HYBRID`, build `libobs`,
`libobs-c-reference`, and `test_image_exif_orientation`, then run the CTest
command above once per library with that library's directory first in
`LD_LIBRARY_PATH`. The version branch is determined by the headers used to
build libobs, not by running the same binary with a different shared library.


## Pool resume on 2026-11-16

Resumed the earlier characterization and extraction on master
`94fa7ce85868c3604cf8fd31a458a4cf641061c7`, using branch
`test/4431-ffmpeg-orientation-pool`. Before extraction, the unchanged
`test_image_exif_orientation` passed against both libraries again: 20 cases
passed and seven PNG cases skipped in each run.

RED commit `d73d4736f93b7790841256e985671099029dabfe` replaces orientation
selection with a zero-returning stub. The same C test failed against both
libraries: JPEG and WebP orientations 2–8 failed (14 failures per library),
with wrong corner colors or unswapped dimensions. Six cases passed and
seven PNG cases skipped. No C test was edited for this check.

The GREEN commit restores the original selection block verbatim in the
extracted file, byte-identical to the earlier extraction at `73c84078d`.
The same CTest command passed against both libraries after extraction:
20 cases passed and seven PNG cases skipped in each run. Before and after,
the hybrid and C-reference dynamic export lists were identical (1,768
symbols). FFmpeg remains 8.0, with libavcodec 62.11.100 and libavutil
60.8.100; the coverage limits above still apply. No Rust, existing C tests,
public headers, test graph, or sibling image initialization/cleanup code
changed. Delivery is branch-only; no PR or auto-merge was created.
