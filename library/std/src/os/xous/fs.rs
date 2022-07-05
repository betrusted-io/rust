#![stable(feature = "rust1", since = "1.0.0")]

use crate::fs;
use crate::sys_common::AsInner;

#[stable(feature = "file_type_ext", since = "1.5.0")]
pub trait FileTypeExt {
    /// Returns `true` if this file type is a basis.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::xous::fs::FileTypeExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata(":.System")?;
    ///     let file_type = meta.file_type();
    ///     assert!(file_type.is_basis());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_type_ext", since = "1.5.0")]
    fn is_basis(&self) -> bool;
}

#[stable(feature = "file_type_ext", since = "1.5.0")]
impl FileTypeExt for fs::FileType {
    fn is_basis(&self) -> bool {
        self.as_inner().is_basis()
    }
}