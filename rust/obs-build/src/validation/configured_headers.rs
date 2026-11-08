//! Deliberately small configured-header contract: literal @NAME@ substitutions.
//! No CMake language evaluation, runtime path expansion or implicit variables.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredHeader {
    pub input: String,
    pub values: BTreeMap<String, String>,
}

impl ConfiguredHeader {
    pub fn validate(&self, output: &str) -> Result<(), String> {
        let valid_path = !self.input.is_empty()
            && !self.input.contains(['\\', ':', ';'])
            && !self.input.chars().any(char::is_control)
            && self
                .input
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."));
        let valid_output = output.ends_with(".h")
            && output
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            && !output.starts_with('.');
        if !valid_path
            || !valid_output
            || self.values.iter().any(|(key, value)| {
                key.is_empty()
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    || value.contains(['@', ';'])
                    || value.chars().any(char::is_control)
            })
        {
            return Err(format!("invalid native test configured header {output:?}"));
        }
        Ok(())
    }

    pub fn render(&self, text: &str) -> String {
        let mut text = text.replace("\r\n", "\n");
        for (name, value) in &self.values {
            text = text.replace(&format!("@{name}@"), value);
        }
        text
    }
}

impl super::Target {
    pub(super) fn configure_headers(
        &self,
        build: &crate::NativeBuild,
        root: &std::path::Path,
        name: &str,
        target: &mut crate::SharedLibrary,
    ) -> Result<Vec<std::path::PathBuf>, String> {
        let mut watched = Vec::new();
        for (output, header) in &self.configured_headers {
            let directory = build
                .output
                .join("native-tests")
                .join(name)
                .join("configured");
            let input = root.join(&header.input);
            let text = std::fs::read_to_string(&input).map_err(|error| error.to_string())?;
            watched.push(input);
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            crate::frontend::write(&directory.join(output), &header.render(&text))?;
            target.include_dirs.push(directory);
        }
        Ok(watched)
    }
}
