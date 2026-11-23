use std::{collections::HashMap, fmt::{Debug, Display, Formatter}, future::Future, path::{Path, PathBuf}, pin::Pin, sync::mpsc::{self, Receiver, Sender}};

use anarchy::{EntityID, World};
use gearbox::{anyhow::self, ui::UiBundle};
use uuid::Uuid;

use crate::{DeltosEditor, SOURCE_UUID_NAMESPACE};

#[cfg(not(target_arch = "wasm32"))]
pub mod local;
pub mod memory;

/// A boxed future that can run on a background task, see `SourceTasks`.
///
/// It is `Send` natively, where `gearbox::run_async` runs it on its own thread, and not `Send`
/// on wasm, where it runs on the browser's event loop and browser futures never are.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A boxed future that can run on a background task, see `SourceTasks`.
///
/// It is `Send` natively, where `gearbox::run_async` runs it on its own thread, and not `Send`
/// on wasm, where it runs on the browser's event loop and browser futures never are.
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// The future returned by every async `Source` method.
pub type SourceFuture<'a, T> = BoxFuture<'a, anyhow::Result<T>>;

/// A change to the world made once a background task finishes, see `SourceTasks`.
pub type TaskResult = Box<dyn FnOnce(&World, &mut DeltosEditor) + Send>;

/// The contents of a folder in a `Source`.
///
/// Fields:
/// * folders: Paths of every folder in the folder.
/// * files: Paths of every file in the folder.
/// * versions: Version (like an ETag) of each file in `files` whose source tracks versions, by
///   path.  Files missing here have no known version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileList {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub versions: HashMap<PathBuf, String>
}

impl FileList {
    /// Creates a list of folders and files without versions.
    ///
    /// # Parameters
    /// * folders: Paths of every folder in the folder.
    /// * files: Paths of every file in the folder.
    pub fn new(folders: Vec<PathBuf>, files: Vec<PathBuf>) -> Self {
        Self { folders, files, versions: HashMap::new() }
    }

    /// The version of a listed file, if its source tracks one.
    ///
    /// # Parameters
    /// * path: Path of the file, as listed in `files`.
    pub fn version(&self, path: &Path) -> Option<&str> {
        self.versions
            .get(path)
            .map(String::as_str)
    }
}

/// Errors a `Source` can return that the editor reacts to.  Find them in a returned
/// `anyhow::Error` with `is_conflict`, `is_unsupported` or `downcast_ref`.
///
/// Variants:
/// * Unsupported: The source doesn't support the operation named in `operation`.
/// * ReadOnly: The source is read only, so it refuses every change.
/// * Conflict: A conditional write was refused because the file at `path` changed since the
///   version the caller expected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    Unsupported { operation: &'static str },
    ReadOnly,
    Conflict { path: PathBuf }
}

impl Display for SourceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { operation } => write!(f, "This source doesn't support {operation}"),
            Self::ReadOnly => write!(f, "This source is read only"),
            Self::Conflict { path } => write!(f, "{} changed remotely", path.display()),
        }
    }
}

impl std::error::Error for SourceError {}

/// Whether an error from a `Source` is a `SourceError::Conflict`, anywhere in its chain.
///
/// # Parameters
/// * err: Error returned by a `Source` method.
pub fn is_conflict(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| matches!(cause.downcast_ref::<SourceError>(), Some(SourceError::Conflict { .. })))
}

/// Whether an error from a `Source` is a `SourceError::Unsupported`, anywhere in its chain.
///
/// # Parameters
/// * err: Error returned by a `Source` method.
pub fn is_unsupported(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| matches!(cause.downcast_ref::<SourceError>(), Some(SourceError::Unsupported { .. })))
}

/// The future of a `Source` operation that the source doesn't support.
fn unsupported<'a, T: 'a>(operation: &'static str) -> SourceFuture<'a, T> {
    Box::pin(async move { Err(SourceError::Unsupported { operation }.into()) })
}

/// An action a host offers in a source's root menu in the file tree (see
/// `Source::root_actions`).  Choosing it sends a `SourceActionActivated` event for the host to
/// handle.
///
/// Fields:
/// * id: ID the host matches in `SourceActionActivated::action`.
/// * label: Text shown in the menu.
/// * icon: Nerd Font glyph shown before the menu label, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAction {
    pub id: String,
    pub label: String,
    pub icon: Option<String>
}

impl SourceAction {
    /// Creates an action without an icon.
    ///
    /// # Parameters
    /// * id: ID the host matches in `SourceActionActivated::action`.
    /// * label: Text shown for the action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None }
    }

    /// Sets the glyph shown with the action.
    ///
    /// # Parameters
    /// * icon: A Nerd Font glyph.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// ECS event sent when a `SourceAction` is chosen in the file tree.  Read it in a system with
/// `anarchy::Event<SourceActionActivated>`.
///
/// Fields:
/// * source: ID of the `Source` whose root menu the action was chosen from.
/// * action: ID of the chosen `SourceAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceActionActivated {
    pub source: Uuid,
    pub action: String
}

/// A source of all files that can be viewed / edited.
///
/// Stored by the `DeltosEditor`, listed by the file selector and read and written by
/// `FileEditor`s.  Every method but the metadata ones (`id`, `name`, `icon`, `read_only`,
/// `order` and `root_actions`) is async, so slow sources (like network ones) never block a
/// frame.  Their futures are usually run with `SourceTasks`.
pub trait Source: Debug + Send + Sync {

    /// The unique ID of this source.
    fn id(&self) -> Uuid { Uuid::new_v5(&SOURCE_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The display name of this source.
    fn name(&self) -> String;

    /// A Nerd Font glyph drawn before the source's name in the file tree, if any.
    fn icon(&self) -> Option<String> { None }

    /// Whether the editor must not change this source's files.  Read-only sources get a lock
    /// badge in the file tree, and their files open non-editable and are never saved.
    fn read_only(&self) -> bool { false }

    /// Sort key of this source among the file tree's roots, lowest first.  Ties sort by name.
    fn order(&self) -> i32 { 0 }

    /// Actions shown in the context menu of this source's root in the file tree.  Choosing one
    /// sends a `SourceActionActivated` event with this source's ID.
    fn root_actions(&self) -> Vec<SourceAction> { Vec::new() }

    /// Async: List all files at a path.
    fn list_files(&self, path: PathBuf) -> SourceFuture<'_, FileList>;

    /// Async: Read a file as text.
    fn read_file_text(&self, path: PathBuf) -> SourceFuture<'_, String>;

    /// Async: Read a file as text along with its current version, if the source tracks
    /// versions.  Editors pass the version back to `write_file_text_if` when saving.  By
    /// default reads with `read_file_text` and returns no version.
    fn read_file_text_versioned(&self, path: PathBuf) -> SourceFuture<'_, (String, Option<String>)> {
        Box::pin(async move { Ok((self.read_file_text(path).await?, None)) })
    }

    /// Async: Read a file as bytes.
    fn read_file_bytes(&self, path: PathBuf) -> SourceFuture<'_, Vec<u8>>;

    /// Async: Write to a file as text.
    fn write_file_text(&self, path: PathBuf, content: String) -> SourceFuture<'_, ()>;

    /// Async: Write to a file as bytes.
    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> SourceFuture<'_, ()>;

    /// Async: Write to a file as text only if its version is still `expected_version`, and
    /// return its new version.  `None` expects no known version, so it always writes.  A file
    /// that changed fails with `SourceError::Conflict`.  By default sources don't track
    /// versions, so this writes with `write_file_text` and returns no version.
    fn write_file_text_if(&self, path: PathBuf, content: String, expected_version: Option<String>) -> SourceFuture<'_, Option<String>> {
        let _ = expected_version;
        Box::pin(async move { self.write_file_text(path, content).await.map(|_| None) })
    }

    /// Async: Write to a file as bytes only if its version is still `expected_version`, see
    /// `write_file_text_if`.
    fn write_file_bytes_if(&self, path: PathBuf, content: Vec<u8>, expected_version: Option<String>) -> SourceFuture<'_, Option<String>> {
        let _ = expected_version;
        Box::pin(async move { self.write_file_bytes(path, content).await.map(|_| None) })
    }

    /// Async: Delete a file.  Fails with `SourceError::Unsupported` by default.
    fn delete_file(&self, path: PathBuf) -> SourceFuture<'_, ()> {
        let _ = path;
        unsupported("deleting files")
    }

    /// Async: Move a file from `from` to `to`.  Fails with `SourceError::Unsupported` by
    /// default.
    fn rename_file(&self, from: PathBuf, to: PathBuf) -> SourceFuture<'_, ()> {
        let _ = (from, to);
        unsupported("renaming files")
    }

    /// Async: Create an empty folder.  Fails with `SourceError::Unsupported` by default.
    fn create_folder(&self, path: PathBuf) -> SourceFuture<'_, ()> {
        let _ = path;
        unsupported("creating folders")
    }
}

/// Runs background work, like `Source` calls, and hands each result back to the world.
///
/// Every task ends with a `TaskResult`, which `apply_finished_tasks` runs on the next render
/// update with access to the world and the `DeltosEditor`.
///
/// Fields:
/// * sender: Sends finished tasks' results, cloned into each task.
/// * receiver: Receives finished tasks' results until they are applied.
#[derive(Debug)]
pub struct SourceTasks {
    sender: Sender<TaskResult>,
    receiver: Receiver<TaskResult>
}

impl Default for SourceTasks {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }
}

impl SourceTasks {
    /// Runs `task` in the background and queues its result to be applied to the world.
    ///
    /// The task runs on its own thread natively and on the browser's event loop on wasm, see
    /// `gearbox::run_async`.
    ///
    /// # Parameters
    /// * task: Future to run, ending with the change to make to the world.
    pub fn spawn(&self, task: BoxFuture<'static, TaskResult>) {
        let sender = self.sender.clone();
        gearbox::run_async(async move {
            // the receiver only closes once the editor is dropped, leaving nothing to update
            let _ = sender.send(task.await);
        });
    }

    /// Takes the results of every task finished since the last call.
    pub fn finished(&self) -> Vec<TaskResult> {
        self.receiver
            .try_iter()
            .collect()
    }
}

/// Replaces a placeholder entity with `bundles`, spawned under the placeholder's parent.
///
/// Background tasks show a placeholder while they run and swap in their result with this.
/// Nothing is spawned when the placeholder is gone, like when its tab was closed or its folder
/// rebuilt before the task finished.
///
/// # Parameters
/// * world: World the placeholder lives in.
/// * placeholder: ID of the placeholder entity, usually reserved with `gearbox::ui::reserve_id`.
/// * bundles: Nodes to spawn in place of the placeholder.
pub fn replace_placeholder(world: &World, placeholder: EntityID, bundles: impl IntoIterator<Item = UiBundle>) {
    let Some(parent) = world.hierarchy().parent(placeholder) else { return };
    world.handle(placeholder).despawn_recursive();
    for bundle in bundles {
        world.spawn_with_parent(Some(parent), bundle.into_entity_builder());
    }
}
