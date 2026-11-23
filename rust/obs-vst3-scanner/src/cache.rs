use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
#[derive(Default)]
pub(super) struct Class {
    pub name: String,
    pub id: String,
    pub path: PathBuf,
    pub plugin_name: String,
    pub discardable: bool,
    #[serde(rename = "isFX")]
    pub is_fx: bool,
}

#[derive(Deserialize, Serialize)]
pub(super) struct Cache {
    #[serde(default)]
    version: u32,
    pub plugins: Vec<Class>,
}

impl Cache {
    pub fn new(plugins: Vec<Class>) -> Self {
        Self {
            version: 1,
            plugins,
        }
    }

    pub fn restore(path: &Path) -> Option<Self> {
        if let Some(value) = read_file(path) {
            // A valid JSON object missing "plugins" is stale, not a broken file.
            return serde_json::from_value(value).ok();
        }
        let backup = backup_path(path);
        fs::rename(&backup, path).ok()?;
        serde_json::from_value(read_file(path)?).ok()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let parent = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(&mut temp, self)?;

        // Publish complete files atomically. A distinct temporary file prevents
        // separate OBS processes from writing into each other's scan output.
        match File::open(path) {
            Ok(mut previous) => {
                let mut backup = tempfile::NamedTempFile::new_in(parent)?;
                io::copy(&mut previous, &mut backup)?;
                backup.persist(backup_path(path))?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        temp.persist(path)?;
        Ok(())
    }
}

fn read_file(path: &Path) -> Option<serde_json::Value> {
    let data = fs::read(path).ok()?;
    serde_json::from_slice(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&data)).ok()
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}
