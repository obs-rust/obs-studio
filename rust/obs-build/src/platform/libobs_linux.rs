use crate::{
    SharedLibrary,
    libobs::{Options, add_package, append_sources},
};
use std::{collections::BTreeMap, path::Path};

pub fn configure(
    target: &str,
    root: &Path,
    options: &Options,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    append_sources(sets, "LIBOBS_LINUX_SOURCES", root, library)?;
    library.file_name = Some("libobs.so.30".into());
    library
        .definitions
        .push(("OBS_INSTALL_PREFIX".into(), Some("\"/usr/local\"".into())));
    library
        .definitions
        .push(("SIMDE_ENABLE_OPENMP".into(), None));
    library.compile_options.extend(
        [
            "-std=gnu17",
            "-fno-strict-aliasing",
            "-fvisibility=hidden",
            "-fopenmp-simd",
        ]
        .map(String::from),
    );
    library
        .link_options
        .extend(["-pthread", "-ldl", "-lm"].map(String::from));
    for package in ["x11", "x11-xcb", "xcb", "uuid"] {
        add_package(library, target, package)?;
    }
    if options.pulseaudio {
        add_package(library, target, "libpulse")?;
        library
            .sources
            .push(root.join("libobs/build/libobs.pulse+.h.c"));
    } else {
        library
            .sources
            .push(root.join("libobs/build/libobs.null-audio+.h.c"));
    }
    if options.gio {
        add_package(library, target, "gio-2.0")?;
    }
    if options.wayland {
        for package in ["wayland-client", "xkbcommon"] {
            add_package(library, target, package)?;
        }
        library
            .sources
            .push(root.join("libobs/build/libobs.nix-wayland+.h.c"));
        if options.gio {
            library
                .sources
                .push(root.join("libobs/build/libobs.nix-portal-shortcuts+.h.c"));
            library
                .definitions
                .push(("HAVE_GLOBAL_SHORTCUTS_PORTAL".into(), None));
        }
    }
    Ok(())
}

pub fn configure_variant(
    root: &Path,
    options: &Options,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
    rust_archive: Option<&Path>,
    config: &Path,
) -> Result<(), String> {
    if let Some(archive) = rust_archive {
        let archive = archive.canonicalize().map_err(|e| e.to_string())?;
        let export_map = config.join("rust-exports.map");
        crate::libobs::write_hidden_list(
            root,
            crate::libobs::HiddenLinker::Elf,
            options.hevc,
            &export_map,
        )?;
        let mut options = vec![
            "-Wl,--whole-archive".into(),
            archive.display().to_string(),
            "-Wl,--no-whole-archive".into(),
            format!("-Wl,--version-script={}", export_map.display()),
        ];
        // rustc --print native-static-libs for this target also requires util,
        // rt and gcc_s; uuid/pthread/m/dl already follow through libobs.
        options.extend(["-lutil", "-lrt", "-lgcc_s"].map(String::from));
        options.append(&mut library.link_options);
        library.link_options = options;
    } else {
        append_sources(sets, "LIBOBS_LINUX_PORTED_SOURCES", root, library)?;
        if options.gio {
            append_sources(sets, "LIBOBS_NIX_GIO_PORTED_SOURCES", root, library)?;
        }
        if options.pulseaudio {
            append_sources(sets, "LIBOBS_PULSE_PORTED_SOURCES", root, library)?;
        }
        if options.wayland {
            append_sources(sets, "LIBOBS_WAYLAND_PORTED_SOURCES", root, library)?;
            if options.gio {
                append_sources(sets, "LIBOBS_WAYLAND_GIO_PORTED_SOURCES", root, library)?;
            }
        }
    }
    Ok(())
}
