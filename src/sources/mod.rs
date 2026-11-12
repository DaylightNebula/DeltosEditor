use std::{fmt::Debug, future::Future, path::PathBuf, pin::Pin, sync::mpsc::{self, Receiver, Sender}};

use anarchy::{EntityID, World};
use gearbox::{anyhow::self, ui::UiBundle};
use uuid::Uuid;

use crate::{DeltosEditor, SOURCE_UUID_NAMESPACE};

pub mod local;

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
pub struct FileList {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>
}

/// A source of all files that can be viewed / edited.
///
/// Stored by the `DeltosEditor`, listed by the file selector and read and written by
/// `FileEditor`s.  Every method but `id` and `name` is async, so slow sources (like network
/// ones) never block a frame.  Their futures are usually run with `SourceTasks`.
pub trait Source: Debug + Send + Sync {

    /// The unique ID of this source.
    fn id(&self) -> Uuid { Uuid::new_v5(&SOURCE_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The display name of this source.
    fn name(&self) -> String;

    /// Async: List all files at a path.
    fn list_files(&self, path: PathBuf) -> SourceFuture<'_, FileList>;

    /// Async: Read a file as text.
    fn read_file_text(&self, path: PathBuf) -> SourceFuture<'_, String>;

    /// Async: Read a file as bytes.
    fn read_file_bytes(&self, path: PathBuf) -> SourceFuture<'_, Vec<u8>>;

    /// Async: Write to a file as text.
    fn write_file_text(&self, path: PathBuf, content: String) -> SourceFuture<'_, ()>;

    /// Async: Write to a file as bytes.
    fn write_file_bytes(&self, path: PathBuf, content: Vec<u8>) -> SourceFuture<'_, ()>;
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
