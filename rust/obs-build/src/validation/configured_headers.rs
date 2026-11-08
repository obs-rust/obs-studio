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
