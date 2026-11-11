//! Which OBS module brought in which libraries (#4525): the owner rule on
//! made-up image lists, the table and its lookup, and the host's own list.

use obs_exit_watchdog::images::{Image, Owner, SUPPORTED, Table, current, file_name, owners};

fn img(start: usize, path: &str) -> Image {
    Image {
        start,
        end: start + 0x1000,
        path: path.to_string(),
    }
}

fn module(name: &str) -> Owner {
    Owner::Module(name.to_string())
}

fn modules(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

#[test]
fn file_names_drop_the_directory() {
    assert_eq!(
        file_name("/usr/lib/obs-plugins/obs-ffmpeg.so"),
        "obs-ffmpeg.so"
    );
    assert_eq!(
        file_name(r"C:\Program Files\obs-studio\obs-plugins\64bit\win-capture.dll"),
        "win-capture.dll"
    );
    assert_eq!(file_name("libobs.so.30"), "libobs.so.30");
    assert_eq!(file_name(""), "");
}

/// The #3739 case: a module loads a sub-plugin and its libraries while it
/// loads, so they come between it and the next module.
#[test]
fn a_modules_libraries_follow_it_in_load_order() {
    let images = [
        img(0x1000, "/usr/bin/obs"),
        img(0x2000, "/usr/lib/libobs.so.30"),
        img(0x3000, "/usr/lib/libQt6Core.so.6"),
        img(0x4000, "/usr/lib/obs-plugins/obs-ffmpeg.so"),
        img(0x5000, "/usr/lib/libavcodec.so.61"),
        img(0x6000, "/usr/lib/obs-plugins/advanced-scene-switcher.so"),
        img(0x7000, "/usr/lib/obs-plugins/adv-ss-plugins/midi.so"),
        img(0x8000, "/usr/lib/libremidi.so.4"),
        img(0x9000, "/usr/lib/obs-plugins/obs-avip.so"),
    ];
    let mods = modules(&[
        "/usr/lib/obs-plugins/obs-ffmpeg.so",
        "/usr/lib/obs-plugins/advanced-scene-switcher.so",
        "/usr/lib/obs-plugins/obs-avip.so",
    ]);
    assert_eq!(
        owners(&images, &mods),
        vec![
            Owner::Obs,
            Owner::Obs,
            Owner::Obs,
            module("obs-ffmpeg.so"),
            module("obs-ffmpeg.so"),
            module("advanced-scene-switcher.so"),
            module("advanced-scene-switcher.so"),
            module("advanced-scene-switcher.so"),
            module("obs-avip.so"),
        ]
    );
}

/// The image list and libobs may spell a module's path differently (a
/// symlink, a relative plugin path): the file name decides. A module that is
/// not in the image list owns nothing.
#[test]
fn modules_are_matched_by_file_name() {
    let images = [
        img(0x1000, ""),
        img(0x2000, "/opt/obs/lib/obs-plugins/linux-v4l2.so"),
        img(0x3000, "/usr/lib/libv4l2.so.0"),
    ];
    let mods = modules(&[
        "../../obs-plugins/64bit/linux-v4l2.so",
        "/gone/never-loaded.so",
    ]);
    assert_eq!(
        owners(&images, &mods),
        vec![Owner::Obs, module("linux-v4l2.so"), module("linux-v4l2.so")]
    );
}

/// macOS: libobs names a plugin by its bundle, the image list by the
/// binary inside it.
#[test]
fn a_macos_bundle_matches_the_binary_inside_it() {
    let images = [
        img(0x1000, "/Applications/OBS.app/Contents/MacOS/OBS"),
        img(
            0x2000,
            "/Users/u/Library/Application Support/obs-studio/plugins/exit-blocker.plugin/Contents/MacOS/exit-blocker",
        ),
        img(0x3000, "/usr/lib/libc++.1.dylib"),
        img(
            0x4000,
            "/Applications/OBS.app/Contents/PlugIns/mac-capture.plugin/Contents/MacOS/mac-capture",
        ),
    ];
    let mods = modules(&[
        "/Users/u/Library/Application Support/obs-studio/plugins/exit-blocker.plugin",
        "/Applications/OBS.app/Contents/PlugIns/mac-capture.plugin",
    ]);
    assert_eq!(
        owners(&images, &mods),
        vec![
            Owner::Obs,
            module("exit-blocker.plugin"),
            module("exit-blocker.plugin"),
            module("mac-capture.plugin"),
        ]
    );
}

#[test]
fn no_modules_means_everything_is_obs() {
    let images = [
        img(0x1000, "/usr/bin/obs"),
        img(0x2000, "/usr/lib/libc.so.6"),
    ];
    assert_eq!(owners(&images, &[]), vec![Owner::Obs, Owner::Obs]);
    assert_eq!(owners(&[], &modules(&["/x/a.so"])), Vec::<Owner>::new());
}

/// Images that appear after loading are marked by when they were first
/// seen. An image found again keeps its first owner; one that was unloaded
/// in between is simply not in the later list.
#[test]
fn the_table_marks_late_images_by_phase() {
    let after_load = [
        img(0x10_000, "/usr/bin/obs"),
        img(0x20_000, "/usr/lib/obs-plugins/obs-browser.so"),
        img(0x30_000, "/usr/lib/libcef.so"),
    ];
    let after_post_load = [
        img(0x10_000, "/usr/bin/obs"),
        img(0x20_000, "/usr/lib/obs-plugins/obs-browser.so"),
        img(0x30_000, "/usr/lib/libcef.so"),
        img(0x40_000, "/usr/lib/libpipewire-0.3.so.0"),
    ];
    let at_arm = [
        img(0x10_000, "/usr/bin/obs"),
        img(0x20_000, "/usr/lib/obs-plugins/obs-browser.so"),
        // libcef.so unloaded, a new library in its place.
        img(0x40_000, "/usr/lib/libpipewire-0.3.so.0"),
        img(0x50_000, "/usr/lib/libnss3.so"),
    ];
    let table = Table::build(
        &after_load,
        &after_post_load,
        &at_arm,
        &modules(&["/usr/lib/obs-plugins/obs-browser.so"]),
    );
    let got: Vec<(usize, &str, &Owner)> = table
        .entries()
        .iter()
        .map(|e| (e.start, e.file.as_str(), &e.owner))
        .collect();
    assert_eq!(
        got,
        vec![
            (0x10_000, "obs", &Owner::Obs),
            (0x20_000, "obs-browser.so", &module("obs-browser.so")),
            (0x30_000, "libcef.so", &module("obs-browser.so")),
            (0x40_000, "libpipewire-0.3.so.0", &Owner::PostLoad),
            (0x50_000, "libnss3.so", &Owner::AfterStartup),
        ]
    );
}

#[test]
fn the_table_is_sorted_by_address_and_finds_ranges() {
    // Load order is not address order.
    let after_load = [
        Image {
            start: 0x9000,
            end: 0xa000,
            path: "/usr/bin/obs".into(),
        },
        Image {
            start: 0x1000,
            end: 0x3000,
            path: "/usr/lib/obs-plugins/a.so".into(),
        },
        Image {
            start: 0x5000,
            end: 0x6000,
            path: "/usr/lib/liba.so".into(),
        },
    ];
    let table = Table::build(
        &after_load,
        &after_load,
        &after_load,
        &modules(&["/p/a.so"]),
    );
    let starts: Vec<usize> = table.entries().iter().map(|e| e.start).collect();
    assert_eq!(starts, vec![0x1000, 0x5000, 0x9000]);
    let file = |a| table.find(a).map(|e| e.file.as_str());
    assert_eq!(file(0x1000), Some("a.so"));
    assert_eq!(file(0x2fff), Some("a.so"));
    assert_eq!(file(0x3000), None, "the end is outside");
    assert_eq!(file(0x5800), Some("liba.so"));
    assert_eq!(file(0x9fff), Some("obs"));
    assert_eq!(file(0x0), None);
    assert_eq!(file(usize::MAX), None);
    assert_eq!(Table::default().find(0x1000), None);
}

/// The host's own list: the main program first, then libraries, without
/// overlaps; a function of this test binary and one of the C library are
/// found in it.
#[test]
fn the_host_lists_its_images_in_load_order() {
    let images = current();
    if !SUPPORTED {
        assert!(images.is_empty());
        return;
    }
    assert!(images.len() > 2, "{images:?}");
    let exe = std::env::current_exe().unwrap();
    let exe_name = exe.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        file_name(&images[0].path),
        exe_name,
        "the main program comes first"
    );
    for image in &images {
        assert!(image.start < image.end, "{image:?}");
    }
    let table = Table::build(&images, &images, &images, &[]);
    for pair in table.entries().windows(2) {
        assert!(pair[0].end <= pair[1].start, "overlap: {pair:?}");
    }
    let here = the_host_lists_its_images_in_load_order as fn() as usize;
    assert_eq!(table.find(here).map(|e| e.file.as_str()), Some(exe_name));
    // A heap address is in no image.
    let heap = Box::new(0u64);
    assert_eq!(table.find(std::ptr::from_ref(&*heap).addr()), None);
    let malloc = libc_malloc_address();
    let owner = table
        .find(malloc)
        .map(|e| e.file.clone())
        .unwrap_or_default();
    assert!(
        owner.starts_with("libc") || owner.starts_with("libsystem"),
        "malloc in {owner:?}"
    );
}

fn libc_malloc_address() -> usize {
    unsafe extern "C" {
        fn malloc(size: usize) -> *mut std::ffi::c_void;
    }
    malloc as unsafe extern "C" fn(usize) -> *mut std::ffi::c_void as usize
}
