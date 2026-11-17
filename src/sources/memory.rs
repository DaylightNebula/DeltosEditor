use std::{collections::HashMap, path::{Component, Path, PathBuf}, sync::{Arc, Mutex}};

use gearbox::anyhow::{self, Context};

use crate::sources::{FileList, Source, SourceAction, SourceError, SourceFuture};

/// Every file of a `MemorySource`: its content and version, by path.
type FileMap = HashMap<PathBuf, (Vec<u8>, u64)>;

/// A `Source` that keeps its files in memory, for the browser, tests and examples.
///
/// Paths are relative to the source's root, which is `.`.  Folders exist implicitly while any
/// file is inside them, plus any made with `create_folder`.  Every write bumps a file's version,
/// so it supports conditional writes like a remote source would.
///
/// Fields:
/// * name: Display name of the source, also used to derive its ID.
/// * icon: Glyph drawn before the source's name in the file tree, if any.
/// * read_only: Whether the editor must not change the source's files.
/// * order: Sort key of the source among the file tree's roots.
/// * actions: Actions shown in the context menu of the source's root.
/// * files: Every file's content and version, by path, shared by every clone.
/// * folders: Folders made with `create_folder`, shared by every clone.
#[derive(Debug, Clone)]
pub struct MemorySource {
    pub name: String,
    pub icon: Option<String>,
    pub read_only: bool,
    pub order: i32,
    pub actions: Vec<SourceAction>,
    files: Arc<Mutex<FileMap>>,
    folders: Arc<Mutex<Vec<PathBuf>>>
}

impl MemorySource {
    /// Creates an empty, writable source.
    ///
    /// # Parameters
    /// * name: Display name of the source, also used to derive its ID.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            icon: None,
            read_only: false,
            order: 0,
            actions: Vec::new(),
            files: Arc::default(),
            folders: Arc::default()
        }
    }

    /// Adds a file to the source, replacing any file at that path.
    ///
    /// # Parameters
    /// * path: Path of the file, relative to the source's root.
    /// * content: Text of the file.
    pub fn with_file(self, path: impl AsRef<Path>, content: impl Into<String>) -> Self {
        if let Ok(mut files) = self.files.lock() {
            let version = files.get(&normalize(path.as_ref())).map_or(1, |(_, version)| version + 1);
            files.insert(normalize(path.as_ref()), (content.into().into_bytes(), version));
        }
        self
    }

    /// Sets the glyph drawn before the source's name in the file tree.
    ///
    /// # Parameters
    /// * icon: A Nerd Font glyph.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets whether the editor must not change the source's files.
    ///
    /// # Parameters
    /// * read_only: True to lock the source.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Sets the sort key of the source among the file tree's roots, lowest first.
    ///
    /// # Parameters
    /// * order: The sort key.
    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Adds an action to the context menu of the source's root.
    ///
    /// # Parameters
    /// * action: The action to add.
    pub fn with_root_action(mut self, action: SourceAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Writes a file, checking its version first when `expected` is given.
    fn write(&self, path: &Path, content: Vec<u8>, expected: Option<&str>) -> anyhow::Result<Option<String>> {
        anyhow::ensure!(!self.read_only, SourceError::ReadOnly);
        let path = normalize(path);
        let mut files = self.files
            .lock()
            .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;

        // refuse the write when the file changed since the caller last saw it
        let current = files.get(&path).map(|(_, version)| version.to_string());
        if let Some(expected) = expected && current.as_deref() != Some(expected) {
            return Err(SourceError::Conflict { path }.into());
        }

        let version = files.get(&path).map_or(1, |(_, version)| version + 1);
        files.insert(path, (content, version));
        Ok(Some(version.to_string()))
    }

    /// Reads a file and its version.
    fn read(&self, path: &Path) -> anyhow::Result<(Vec<u8>, String)> {
        let files = self.files
            .lock()
            .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;
        files.get(&normalize(path))
            .map(|(content, version)| (content.clone(), version.to_string()))
            .with_context(|| format!("{} has no file {}", self.name, path.display()))
    }
}

impl Source for MemorySource {
    fn name(&self) -> String { self.name.clone() }

    fn icon(&self) -> Option<String> { self.icon.clone() }

    fn read_only(&self) -> bool { self.read_only }

    fn order(&self) -> i32 { self.order }

    fn root_actions(&self) -> Vec<SourceAction> { self.actions.clone() }

    fn list_files(&self, path: PathBuf) -> SourceFuture<'_, FileList> {
        Box::pin(async move {
            let folder = normalize(&path);
            let files = self.files
                .lock()
                .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;
            let made = self.folders
                .lock()
                .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;

            // sort every known path into the folder's direct files and child folders
            let mut list = FileList::default();
            for (file, (_, version)) in files.iter() {
                let Ok(rest) = file.strip_prefix(&folder) else { continue };
                let mut parts = rest.components();
                let Some(first) = parts.next() else { continue };
                if parts.next().is_none() {
                    list.files.push(file.clone());
                    list.versions.insert(file.clone(), version.to_string());
                } else {
                    let child = folder.join(first);
                    if !list.folders.contains(&child) { list.folders.push(child) }
                }
            }
            for made in made.iter() {
                if made.parent() == Some(folder.as_path()) && !list.folders.contains(made) {
                    list.folders.push(made.clone());
                }
            }

            list.folders.sort();
            list.files.sort();
            Ok(list)
        })
    }

    fn read_file_text(&self, path: PathBuf) -> SourceFuture<'_, String> {
        Box::pin(async move {
            let (content, _) = self.read(&path)?;
            String::from_utf8(content).with_context(|| format!("{} isn't text", path.display()))
        })
    }

    fn read_file_text_versioned(&self, path: PathBuf) -> SourceFuture<'_, (String, Option<String>)> {
        Box::pin(async move {
            let (content, version) = self.read(&path)?;
            let text = String::from_utf8(content).with_context(|| format!("{} isn't text", path.display()))?;
            Ok((text, Some(version)))
        })
    }

    fn read_file_bytes(&self, path: PathBuf) -> SourceFuture<'_, Vec<u8>> {
        Box::pin(async move { Ok(self.read(&path)?.0) })
    }

    fn write_file_text(&self, path: PathBuf, content: String) -> SourceFuture<'_, ()> {
        Box::pin(async move { self.write(&path, content.into_bytes(), None).map(|_| ()) })
    }

    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> SourceFuture<'_, ()> {
        Box::pin(async move { self.write(&path, content, None).map(|_| ()) })
    }

    fn write_file_text_if(&self, path: PathBuf, content: String, expected_version: Option<String>) -> SourceFuture<'_, Option<String>> {
        Box::pin(async move { self.write(&path, content.into_bytes(), expected_version.as_deref()) })
    }

    fn write_file_bytes_if(&self, path: PathBuf, content: Vec<u8>, expected_version: Option<String>) -> SourceFuture<'_, Option<String>> {
        Box::pin(async move { self.write(&path, content, expected_version.as_deref()) })
    }

    fn delete_file(&self, path: PathBuf) -> SourceFuture<'_, ()> {
        Box::pin(async move {
            anyhow::ensure!(!self.read_only, SourceError::ReadOnly);
            let mut files = self.files
                .lock()
                .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;
            files.remove(&normalize(&path))
                .map(|_| ())
                .with_context(|| format!("{} has no file {}", self.name, path.display()))
        })
    }

    fn rename_file(&self, from: PathBuf, to: PathBuf) -> SourceFuture<'_, ()> {
        Box::pin(async move {
            anyhow::ensure!(!self.read_only, SourceError::ReadOnly);
            let mut files = self.files
                .lock()
                .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;
            let (content, version) = files.remove(&normalize(&from))
                .with_context(|| format!("{} has no file {}", self.name, from.display()))?;
            files.insert(normalize(&to), (content, version + 1));
            Ok(())
        })
    }

    fn create_folder(&self, path: PathBuf) -> SourceFuture<'_, ()> {
        Box::pin(async move {
            anyhow::ensure!(!self.read_only, SourceError::ReadOnly);
            let mut folders = self.folders
                .lock()
                .map_err(|_| anyhow::anyhow!("{} is poisoned", self.name))?;
            let path = normalize(&path);
            if !folders.contains(&path) { folders.push(path) }
            Ok(())
        })
    }
}

/// Makes a path relative to the source's root `.`, so `a.txt`, `./a.txt` and `/a.txt` match.
fn normalize(path: &Path) -> PathBuf {
    let rest: PathBuf = path.components()
        .filter(|part| matches!(part, Component::Normal(_)))
        .collect();
    Path::new(".").join(rest)
}

#[cfg(test)]
mod tests {
    use std::{future::Future, pin::pin, task::{Context, Poll, Waker}};

    use super::*;
    use crate::sources::is_conflict;

    /// Runs a future that finishes on its first poll, which every `MemorySource` future does.
    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("MemorySource future didn't finish on its first poll"),
        }
    }

    #[test]
    fn lists_direct_children_only() -> anyhow::Result<()> {
        let source = MemorySource::new("mem")
            .with_file("Cargo.toml", "")
            .with_file("src/main.rs", "");

        let root = ready(source.list_files(".".into()))?;
        let src = ready(source.list_files("./src".into()))?;

        assert_eq!(root.files, vec![PathBuf::from("./Cargo.toml")]);
        assert_eq!(root.folders, vec![PathBuf::from("./src")]);
        assert_eq!(src.files, vec![PathBuf::from("./src/main.rs")]);
        assert_eq!(src.version(Path::new("./src/main.rs")), Some("1"));
        Ok(())
    }

    #[test]
    fn stale_versions_conflict() -> anyhow::Result<()> {
        let source = MemorySource::new("mem").with_file("a.txt", "one");
        let (_, version) = ready(source.read_file_text_versioned("a.txt".into()))?;

        let next = ready(source.write_file_text_if("a.txt".into(), "two".into(), version.clone()))?;
        let stale = ready(source.write_file_text_if("a.txt".into(), "three".into(), version))
            .expect_err("stale write");

        assert_eq!(next.as_deref(), Some("2"));
        assert!(is_conflict(&stale));
        assert_eq!(ready(source.read_file_text("a.txt".into()))?, "two");
        Ok(())
    }

    #[test]
    fn read_only_refuses_writes() {
        let source = MemorySource::new("mem")
            .with_file("a.txt", "one")
            .with_read_only(true);

        assert!(ready(source.write_file_text("a.txt".into(), "two".into())).is_err());
        assert!(ready(source.delete_file("a.txt".into())).is_err());
    }
}
