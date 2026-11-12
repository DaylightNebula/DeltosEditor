use std::path::PathBuf;

use gearbox::anyhow::Context;

use crate::sources::{FileList, Source, SourceFuture};

/// A translator `FileSource for any `neb_fs::FileSystem`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalFileSource;

impl Source for LocalFileSource {

    fn name(&self) -> String {
        "Local Files".to_string()
    }

    fn list_files(&self, path: PathBuf) -> SourceFuture<'_, FileList> {
        Box::pin(async move {
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
        })
    }

    fn read_file_text(&self, path: PathBuf) -> SourceFuture<'_, String> {
        Box::pin(async move {
            std::fs::read_to_string(&path).with_context(|| format!("Couldn't read {}", path.display()))
        })
    }

    fn read_file_bytes(&self, path: PathBuf) -> SourceFuture<'_, Vec<u8>> {
        Box::pin(async move {
            std::fs::read(&path).with_context(|| format!("Couldn't read {}", path.display()))
        })
    }

    fn write_file_text(&self, path: PathBuf, content: String) -> SourceFuture<'_, ()> {
        self.write_file_bytes(path, content.into_bytes())
    }

    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> SourceFuture<'_, ()> {
        Box::pin(async move {
            std::fs::write(&path, content).with_context(|| format!("Couldn't write {}", path.display()))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{future::Future, path::Path, pin::pin, task::{Context, Poll, Waker}};

    use gearbox::anyhow;
    use uuid::Uuid;

    use super::*;

    /// Runs a future that finishes on its first poll, which every `LocalFileSource` future does.
    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("LocalFileSource future didn't finish on its first poll"),
        }
    }

    /// Makes an empty folder in the temp directory that is unique to one test.
    fn temp_folder() -> PathBuf {
        let folder = std::env::temp_dir().join(format!("deltos-local-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&folder).expect("temp folder is created");
        folder
    }

    #[test]
    fn text_round_trips() -> anyhow::Result<()> {
        let folder = temp_folder();
        let path = folder.join("note.txt");

        ready(LocalFileSource.write_file_text(path.clone(), "hello".into()))?;
        assert_eq!(ready(LocalFileSource.read_file_text(path.clone()))?, "hello");
        assert_eq!(ready(LocalFileSource.read_file_bytes(path))?, b"hello");

        std::fs::remove_dir_all(folder)?;
        Ok(())
    }

    #[test]
    fn lists_folders_and_files_apart() -> anyhow::Result<()> {
        let folder = temp_folder();
        std::fs::create_dir(folder.join("inner"))?;
        ready(LocalFileSource.write_file_bytes(folder.join("a.bin"), vec![1, 2, 3]))?;

        let list = ready(LocalFileSource.list_files(folder.clone()))?;

        assert_eq!(list.folders, vec![folder.join("inner")]);
        assert_eq!(list.files, vec![folder.join("a.bin")]);
        std::fs::remove_dir_all(folder)?;
        Ok(())
    }

    #[test]
    fn missing_paths_error_with_their_path() {
        let missing = Path::new("definitely/not/here.txt").to_path_buf();

        let read = ready(LocalFileSource.read_file_text(missing.clone())).expect_err("missing file read");
        let Err(list) = ready(LocalFileSource.list_files(missing.clone())) else { panic!("missing folder listed") };

        assert!(format!("{read:#}").contains(&missing.display().to_string()));
        assert!(format!("{list:#}").contains(&missing.display().to_string()));
    }
}
