use std::path::Path;

pub(crate) fn portable_input(input: &Path) -> String {
    // Depfiles and native paths can use either separator; diagnostics use '/'.
    input.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_input_uses_portable_separators() {
        for input in [
            "rust/obs-core/src/audio_render.rs",
            r"rust\obs-core\src\audio_render.rs",
        ] {
            assert_eq!(
                portable_input(Path::new(input)),
                "rust/obs-core/src/audio_render.rs"
            );
        }
    }
}
