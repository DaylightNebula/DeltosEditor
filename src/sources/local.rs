use std::path::PathBuf;

use gearbox::anyhow::{self, Context};

use crate::sources::{FileList, Source};

/// A translator `FileSource for any `neb_fs::FileSystem`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalFileSource;

impl Source for LocalFileSource {

    fn name(&self) -> String {
        "Local Files".to_string()
    }

    fn list_files(&self, path: PathBuf) -> anyhow::Result<FileList> {
        let entries = std::fs::read_dir(&path).with_context(|| format!("Couldn't list {}", path.display()))?;

        let mut folders = Vec::new();
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.with_context(|| format!("Couldn't list {}", path.display()))?;
            let entry_path = entry.path();
            // `Path::is_dir` follows symlinks, so a link to a folder lists as a folder
            if entry_path.is_dir() {
                folders.push(entry_path);
            } else {
                files.push(entry_path);
            }
        }

        // folders.sort();
        // files.sort_by(|a, b| (&a., &a.extension).cmp(&(&b.name, &b.extension)));
        Ok(FileList { folders, files })
    }

    fn read_file_text(&self, path: PathBuf) -> anyhow::Result<String> {
        std::fs::read_to_string(&path).with_context(|| format!("Couldn't read {}", path.display()))
    }

    fn read_file_bytes(&self, path: PathBuf) -> anyhow::Result<Vec<u8>> {
        std::fs::read(&path).with_context(|| format!("Couldn't read {}", path.display()))
    }

    fn write_file_text(&self, path: PathBuf, content: String) -> anyhow::Result<()> {
        self.write_file_bytes(path, content.into_bytes())
    }

    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> anyhow::Result<()> {
        std::fs::write(&path, content).with_context(|| format!("Couldn't write {}", path.display()))
    }
}
