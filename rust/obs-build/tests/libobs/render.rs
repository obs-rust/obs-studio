//! Uses the unchanged C headers for graphics/video structs and callbacks.
use obs_build::{NativeBuild, SharedLibrary};
use std::path::{Path, PathBuf};

pub fn build(build: &NativeBuild, root: &Path, core: &Path) -> PathBuf {
    std::fs::create_dir_all(&build.output).unwrap();
    let source = build.output.join("graphics-probe.c");
    if std::fs::read_to_string(&source).ok().as_deref() != Some(SOURCE) {
        std::fs::write(&source, SOURCE).unwrap();
    }
    let mut library = SharedLibrary {
        name: "graphics-probe".into(),
        sources: vec![source],
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            build.output.join("config"),
        ],
        dependencies: vec![core.into()],
        compile_options: vec!["-std=gnu17".into()],
        ..SharedLibrary::default()
    };
    for package in ["simde", "x11"] {
        let dependency = pkg_config::Config::new()
            .cargo_metadata(false)
            .probe(package)
            .unwrap();
        library.include_dirs.extend(dependency.include_paths);
        for path in dependency.link_paths {
            library.link_options.push(format!("-L{}", path.display()));
        }
        library
            .link_options
            .extend(dependency.libs.iter().map(|name| format!("-l{name}")));
    }
    build.shared_library(&library).unwrap()
}

const SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>

static Display *test_display;
static atomic_uint frames;
static atomic_uint invalid_frames;
static atomic_uint expected_y = 16;
static uint32_t fixture_width = 64, fixture_height = 64;

EXPORT bool obs_fixture_open_display(void)
{
    XInitThreads();
    test_display = XOpenDisplay(NULL);
    if (!test_display)
        return false;
    obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
    obs_set_nix_platform_display(test_display);
    return true;
}

static void receive_frame(void *unused, struct video_data *frame)
{
    (void)unused;
    if (frame->data[0] && atomic_load(&expected_y) == 235 &&
        atomic_load(&frames) == 0 && frame->data[0][0] == 16)
        return; /* Drain black frames already in the video pipeline. */
    if (!frame->data[0] || frame->linesize[0] < fixture_width || frame->data[0][0] != atomic_load(&expected_y))
        atomic_fetch_add(&invalid_frames, 1);
    atomic_fetch_add(&frames, 1);
}

static int record_clip(const char *path, const char *output_id)
{
    obs_data_t *settings = obs_data_create();
    obs_data_set_string(settings, "rate_control", "CBR");
    obs_data_set_int(settings, "bitrate", 500);
    bool nvenc = getenv("OBS_BUILD_NVENC_ENCODE") != NULL;
    if (!nvenc) {
        obs_data_set_string(settings, "preset", "ultrafast");
        obs_data_set_string(settings, "tune", "zerolatency");
    }
    obs_encoder_t *video = obs_video_encoder_create(nvenc ? "obs_nvenc_h264_soft" : "obs_x264",
        "cc-build-video", settings, NULL);
    obs_data_release(settings);
    settings = obs_data_create();
    obs_data_set_int(settings, "bitrate", 128);
    obs_encoder_t *audio = obs_audio_encoder_create("ffmpeg_aac", "cc-build-audio", settings, 0, NULL);
    obs_data_release(settings);
    settings = obs_data_create();
    obs_data_set_string(settings, "path", path);
    obs_output_t *output = obs_output_create(output_id, "cc-build-recording", settings, NULL);
    obs_data_release(settings);
    int status = 0;
    if (!video || !audio || !output) {
        status = 85;
        goto cleanup;
    }
    obs_encoder_set_video(video, obs_get_video());
    obs_encoder_set_audio(audio, obs_get_audio());
    obs_output_set_video_encoder(output, video);
    obs_output_set_audio_encoder(output, audio, 0);
    if (!obs_output_start(output)) {
        blog(LOG_ERROR, "cc::Build recording start failed: %s", obs_output_get_last_error(output));
        status = 86;
        goto cleanup;
    }
    for (unsigned i = 0; i < 400 && obs_output_active(output) && obs_output_get_total_frames(output) < 15; ++i)
        os_sleep_ms(10);
    if (obs_output_get_total_frames(output) < 15)
        status = 87;
    obs_output_stop(output);
    for (unsigned i = 0; i < 500 && obs_output_active(output); ++i)
        os_sleep_ms(10);
    if (obs_output_active(output)) {
        obs_output_force_stop(output);
        status = 88;
    }
    blog(LOG_INFO, "cc::Build recording: %s (status %d)", path, status);
cleanup:
    obs_output_release(output);
    obs_encoder_release(video);
    obs_encoder_release(audio);
    return status;
}

EXPORT int obs_fixture_initialize(const char *renderer, const char *data_path, bool audio_enabled)
{
    // Hardware encoders have minimum dimensions above the tiny software probe.
    if (getenv("OBS_BUILD_NVENC_ENCODE")) {
        fixture_width = 640;
        fixture_height = 360;
    }
    obs_add_data_path(data_path);
    if (audio_enabled) {
        struct obs_audio_info audio = { .samples_per_sec = 48000, .speakers = SPEAKERS_STEREO };
        if (!obs_reset_audio(&audio))
            return 84;
    }
    struct obs_video_info video = {
        .graphics_module = renderer,
        .fps_num = 30, .fps_den = 1,
        .base_width = fixture_width, .base_height = fixture_height,
        .output_width = fixture_width, .output_height = fixture_height,
        .output_format = VIDEO_FORMAT_NV12,
        .gpu_conversion = true,
        .colorspace = VIDEO_CS_709,
        .range = VIDEO_RANGE_PARTIAL,
        .scale_type = OBS_SCALE_BICUBIC,
    };
    int result = obs_reset_video(&video);
    return result;
}

EXPORT int obs_fixture_render(const char *record_path, const char *output_id)
{
    obs_enter_graphics();
    blog(LOG_INFO, "cc::Build graphics device: %s", gs_get_device_name());
    obs_leave_graphics();
    obs_scene_t *scene = obs_scene_create("cc-build-rendering");
    if (!scene)
        return 81;
    obs_set_output_source(0, obs_scene_get_source(scene));
    atomic_store(&frames, 0);
    atomic_store(&invalid_frames, 0);
    obs_add_raw_video_callback(NULL, receive_frame, NULL);
    for (unsigned i = 0; i < 200 && atomic_load(&frames) < 3; ++i)
        os_sleep_ms(10);
    obs_remove_raw_video_callback(receive_frame, NULL);
    int source_status = 0;
    obs_source_t *color = NULL;
    if (getenv("OBS_BUILD_SOURCES") || getenv("OBS_BUILD_APP_GUI")) {
        obs_data_t *settings = obs_data_create();
        obs_data_set_int(settings, "width", fixture_width);
        obs_data_set_int(settings, "height", fixture_height);
        obs_data_set_int(settings, "color", 0xFFFFFFFF);
        color = obs_source_create("color_source_v3", "cc-build-white", settings, NULL);
        obs_data_release(settings);
        if (!color || obs_source_get_width(color) != fixture_width || obs_source_get_height(color) != fixture_height) {
            source_status = 89;
        } else {
            obs_scene_add(scene, color);
            atomic_store(&frames, 0);
            atomic_store(&expected_y, 235);
            obs_add_raw_video_callback(NULL, receive_frame, NULL);
            for (unsigned i = 0; i < 200 && atomic_load(&frames) < 3; ++i)
                os_sleep_ms(10);
            obs_remove_raw_video_callback(receive_frame, NULL);
            blog(LOG_INFO, "cc::Build color source rendered %u white frames", atomic_load(&frames));
        }
    }
    int recording_status = record_path ? record_clip(record_path, output_id) : 0;
    obs_set_output_source(0, NULL);
    if (color) {
        obs_source_remove(color);
        obs_source_release(color);
    }
    // Public sources also belong to the main canvas until explicitly removed.
    obs_source_remove(obs_scene_get_source(scene));
    obs_scene_release(scene);
    // Match frontend scene teardown: queued scene destruction can release its
    // child sources. Finish it before obs_shutdown force-frees remaining ones.
    while (obs_wait_for_destroy_queue()) {}
    /* The owning test calls obs_shutdown after this probe. */
    if (atomic_load(&frames) < 3)
        return 82;
    if (atomic_load(&invalid_frames))
        return 83;
    return source_status ? source_status : recording_status;
}

EXPORT void obs_fixture_close_display(void)
{
    obs_set_nix_platform_display(NULL);
    XCloseDisplay(test_display);
    test_display = NULL;
}
"#;
