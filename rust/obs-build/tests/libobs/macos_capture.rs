//! Load original capture modules and inspect registration without opening devices.
use obs_build::{NativeBuild, SharedLibrary, libobs::Options};
use std::path::{Path, PathBuf};

#[expect(
    clippy::too_many_lines,
    reason = "one block per captured output, in build order"
)]
pub(super) fn build_probe(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
    core: &Path,
) -> PathBuf {
    let source = build.output.join("config/macos-capture-probe.c");
    std::fs::write(
        &source,
        r#"
#include <obs.h>
#include <CoreFoundation/CoreFoundation.h>
#include <dlfcn.h>
#include <limits.h>
#include <sys/stat.h>
#include <string.h>
#include <stdio.h>

static int same_file(const char *left, const char *right) {
    struct stat a, b;
    return !stat(left, &a) && !stat(right, &b) && a.st_dev == b.st_dev && a.st_ino == b.st_ino;
}

int main(int argc, char **argv) {
    if (argc != 5) return 10;
    if (obs_get_version() != LIBOBS_API_VER) return 11;
    void *version = dlsym(RTLD_DEFAULT, "obs_get_version");
    Dl_info core;
    if (!version || !dladdr(version, &core) || !same_file(core.dli_fname, argv[4])) return 12;
    const char *description;
    const char *expected[6];
    size_t count;
    if (!strcmp(argv[2], "mac-capture")) {
        description = "macOS audio input/output and window/display capture";
        expected[0] = "coreaudio_input_capture"; expected[1] = "coreaudio_output_capture";
        expected[2] = "display_capture"; expected[3] = "window_capture";
        expected[4] = "screen_capture"; expected[5] = "sck_audio_capture";
        count = 6;
    } else if (!strcmp(argv[2], "mac-avcapture")) {
        description = "macOS AVFoundation Capture Source";
        expected[0] = "macos-avcapture"; expected[1] = "macos-avcapture-fast";
        count = 2;
    } else if (!strcmp(argv[2], "mac-avcapture-legacy")) {
        description = "MacOS AVFoundation Capture source";
        expected[0] = "av_capture_input"; expected[1] = "av_capture_input_v2";
        count = 2;
    } else return 13;
    void *handle = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!handle) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 15; }
    typedef const char *(*description_fn)(void);
    typedef uint32_t (*version_fn)(void);
    void *description_export = dlsym(handle, "obs_module_description");
    description_fn describe = (description_fn)description_export;
    version_fn module_version = (version_fn)dlsym(handle, "obs_module_ver");
    if (!describe || strcmp(describe(), description) || !module_version ||
        module_version() != LIBOBS_API_VER) return 14;
    Dl_info owner;
    if (!dladdr(description_export, &owner) || !same_file(owner.dli_fname, argv[1])) return 16;
    CFURLRef data_url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)argv[3], strlen(argv[3]), true);
    CFURLRef contents_url = data_url ? CFURLCreateCopyDeletingLastPathComponent(NULL, data_url) : NULL;
    CFURLRef bundle_url = contents_url ? CFURLCreateCopyDeletingLastPathComponent(NULL, contents_url) : NULL;
    CFBundleRef bundle = bundle_url ? CFBundleCreate(NULL, bundle_url) : NULL;
    CFURLRef executable_url = bundle ? CFBundleCopyExecutableURL(bundle) : NULL;
    UInt8 executable_path[PATH_MAX];
    char identifier[128];
    snprintf(identifier, sizeof(identifier), "com.obsproject.%s", argv[2]);
    CFStringRef expected_identifier = CFStringCreateWithCString(NULL, identifier, kCFStringEncodingUTF8);
    if (!executable_url || !CFURLGetFileSystemRepresentation(executable_url, true, executable_path, sizeof(executable_path)) ||
        !same_file((const char *)executable_path, argv[1]) || !CFBundleGetIdentifier(bundle) ||
        !CFEqual(CFBundleGetIdentifier(bundle), expected_identifier)) {
        fprintf(stderr, "bundle executable/identifier mismatch for %s\n", argv[1]);
        return 23;
    }
    CFRelease(expected_identifier);
    CFRelease(executable_url);
    CFRelease(bundle);
    CFRelease(bundle_url);
    CFRelease(contents_url);
    CFRelease(data_url);
    if (!obs_startup("en-US", NULL, NULL)) return 17;
    size_t baseline = 0;
    const char *id;
    while (obs_enum_input_types(baseline, &id)) baseline++;
    obs_module_t *module = NULL;
    if (obs_open_module(&module, argv[1], argv[3]) != MODULE_SUCCESS || !obs_init_module(module)) return 18;
    if (strcmp(obs_get_module_description(module), description) ||
        !same_file(obs_get_module_binary_path(module), argv[1])) return 19;
    bool found[6] = {false};
    size_t registered = 0;
    for (size_t i = 0; obs_enum_input_types(i, &id); i++) {
        registered++;
        for (size_t j = 0; j < count; j++) {
            if (!strcmp(id, expected[j])) found[j] = true;
        }
    }
    if (registered != baseline + count) return 20;
    for (size_t j = 0; j < count; j++) {
        if (!found[j]) { fprintf(stderr, "missing source: %s\n", expected[j]); return 21; }
        const char *name = obs_source_get_display_name(expected[j]);
        if (!name || !*name) return 22;
    }
    printf("core: %s; capture: %s; description: %s; sources: %zu\n",
           core.dli_fname, owner.dli_fname, description, registered - baseline);
    obs_shutdown();
    dlclose(handle);
    return 0;
}
"#,
    )
    .unwrap();
    build
        .executable(&SharedLibrary {
            name: "macos-capture-probe".into(),
            sources: vec![source],
            include_dirs: vec![
                root.into(),
                root.join("libobs"),
                build.output.join("config"),
                options
                    .native_dependencies
                    .as_ref()
                    .unwrap()
                    .join("include"),
            ],
            dependencies: vec![core.into()],
            c_compile_options: vec!["-std=gnu17".into()],
            compile_options: vec!["-mmacosx-version-min=13.0".into()],
            link_options: vec!["-framework".into(), "CoreFoundation".into()],
            ..SharedLibrary::default()
        })
        .unwrap()
}
