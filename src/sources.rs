use std::{fmt::Debug, path::PathBuf};

use gearbox::anyhow::self;
use uuid::Uuid;

use crate::SOURCE_UUID_NAMESPACE;

pub mod local;

pub struct FileList {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>
}

/// A source of all files that can be viewed / edited.
pub trait Source: Debug + Send {

    /// The unique ID of this source.
    fn id(&self) -> Uuid { Uuid::new_v5(&SOURCE_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The display name of this source.
    fn name(&self) -> String;

    /// Async: List all files at a path.
    fn list_files(&self, path: PathBuf) -> anyhow::Result<FileList>;

    /// Async: Read a file as text.
    fn read_file_text(&self, path: PathBuf) -> anyhow::Result<String>;

    /// Async: Read a file as bytes.
    fn read_file_bytes(&self, path: PathBuf) -> anyhow::Result<Vec<u8>>;

    /// Async: Write to a file as text.
    fn write_file_text(&self, path: PathBuf, content: String) -> anyhow::Result<()>;

    /// Async: Write to a file as bytes.
    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> anyhow::Result<()>;
}
