use std::{fmt::Debug, path::PathBuf, sync::Arc};

use gearbox::ui::UiBundle;
use uuid::Uuid;

use crate::{EDITOR_UUID_NAMESPACE, sources::{BoxFuture, Source}, theme::Theme};

pub mod text;

/// Defines an editor for a specific editor.
pub trait FileEditor: Debug + Send {
    /// The ID of this `FileEditor` type.  By default this is the v5 uuid built from the hash of `name`.
    fn id(&self) -> Uuid { Uuid::new_v5(&EDITOR_UUID_NAMESPACE, self.name().as_bytes()) }

    /// The display name of this `FileEditor`.
    fn name(&self) -> String;

    /// A list of file types this `FileEditor` implementation works on.
    fn file_types(&self) -> Vec<String>;

    /// Supplier of a future to a bundle that represents the editor of a file.
    ///
    /// The future runs in the background, so it can load the file from `source`.  The
    /// `DeltosEditor` shows a placeholder until it finishes.
    fn build_editor(&mut self, theme: &Theme, source: Arc<dyn Source>, path: PathBuf) -> BoxFuture<'static, UiBundle>;
}
