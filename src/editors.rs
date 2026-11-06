use std::path::PathBuf;

use gearbox::ui::UiBundle;

use crate::sources::FileSource;

pub mod text;

/// Defines an editor for a specific editor.
pub trait FileEditor {
    /// The display name of this `FileEditor`.
    fn name(&self) -> String;

    /// A list of file types this `FileEditor` implementation works on.
    fn file_types(&self) -> Vec<String>;

    /// Supplier of a future to a bundle that represents the editor of a file.
    fn build_editor<S: FileSource>(&mut self, source: S, path: PathBuf) -> UiBundle;
}
