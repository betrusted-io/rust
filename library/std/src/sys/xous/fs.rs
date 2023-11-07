use alloc::str::FromStr;

use crate::ffi::OsString;
use crate::fmt;
use crate::hash::Hash;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::os::xous::ffi::{blocking_scalar, lend_mut, OsStrExt};
use crate::os::xous::services::{pddb_server, PddbBlockingScalar, PddbLend, PddbLendMut};
use crate::path::{Path, PathBuf};
use crate::sys::time::SystemTime;
use crate::sys::unsupported;

use super::senres::{self, Senres, SenresMut};

pub use crate::sys_common::fs::try_exists;

pub struct File {
    fd: u16,
    len: u64,
}

#[derive(Clone)]
pub struct FileAttr {
    pub(crate) kind: FileType,
    pub(crate) len: u64,
}

pub struct ReadDir {
    root: PathBuf,
    entries: Vec<DirEntry>,
}

pub struct DirEntry {
    name: String,
    path: String,
    // basis: Option<String>,
    kind: FileType,
}

#[derive(Clone, Debug)]
pub struct OpenOptions {
    create_file: bool,
    append: bool,
    truncate: bool,
    create_new: bool,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {}

#[derive(PartialEq, Eq, Debug, Copy, Clone, Hash)]
pub enum FileType {
    Basis = 0,
    Dict = 1,
    Key = 2,
    /// This represents both a dict and a key that share the same name
    DictKey = 3,
    None = 4,
    Unknown = 5,
}

#[derive(Debug)]
pub struct DirBuilder {}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.len
    }

    pub fn perm(&self) -> FilePermissions {
        FilePermissions {}
    }

    pub fn file_type(&self) -> FileType {
        self.kind
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        // println!("rust: FileAttr::copy()");
        unsupported()
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        // println!("rust: FileAttr::accessed()");
        unsupported()
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        // println!("rust: FileAttr::created()");
        unsupported()
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        false
    }

    pub fn set_readonly(&mut self, _readonly: bool) {}
}

impl FileTimes {
    pub fn set_accessed(&mut self, _t: SystemTime) {}
    pub fn set_modified(&mut self, _t: SystemTime) {}
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        let is_dir = match *self {
            FileType::Basis | FileType::Dict | FileType::DictKey => true,
            FileType::Key | FileType::Unknown | FileType::None => false,
        };
        // println!("rust: {:?} is_dir()? {:?}", self, is_dir);
        is_dir
    }

    pub fn is_file(&self) -> bool {
        let is_file = match *self {
            FileType::DictKey | FileType::Key => true,
            FileType::Basis | FileType::Dict | FileType::Unknown | FileType::None => false,
        };
        // println!("rust: {:?} is_file()? {:?}", self, is_file);
        is_file
    }

    pub fn is_symlink(&self) -> bool {
        false
    }

    pub fn is_basis(&self) -> bool {
        *self == FileType::Basis
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        self.entries.pop().map(|v| Ok(v))
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This will only be called from std::fs::ReadDir, which will add a "ReadDir()" frame.
        // Thus the result will be e g 'ReadDir("C:\")'
        fmt::Debug::fmt(&*self.root, f)
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        PathBuf::from_str(&self.path).unwrap()
    }

    pub fn file_name(&self) -> OsString {
        crate::ffi::OsStr::from_bytes(self.name.as_bytes()).to_os_string()
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        match self.kind {
            FileType::None | FileType::Unknown => Err(crate::io::Error::new(
                crate::io::ErrorKind::NotFound,
                "File or directory does not exist, or is corrupted",
            )),
            _ => Ok(FileAttr { kind: self.kind, len: 0 }),
        }
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.kind)
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions { create_file: false, truncate: false, append: false, create_new: false }
    }

    pub fn read(&mut self, _read: bool) {}
    pub fn write(&mut self, _write: bool) {}
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    pub fn create(&mut self, create: bool) {
        self.create_file = create;
    }
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        let mut request = senres::Stack::<4096>::new();
        let path_as_str = path.as_os_str().to_str().ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;

        {
            let mut writer = request.writer(*b"KyOQ").ok_or_else(|| {
                crate::io::Error::new(
                    crate::io::ErrorKind::InvalidFilename,
                    "unable to create request",
                )
            })?;

            writer.append(path_as_str);
            writer.append(opts.create_file);
            writer.append(false); // create_path
            writer.append(opts.create_new);
            writer.append(opts.append);
            writer.append(opts.truncate);
            writer.append(0u64); // alloc_hint
            writer.append::<Option<[u32; 4]>>(None); // callback SID
        }

        // Make the actual call
        let (err, _) =
            request.lend_mut(pddb_server(), PddbLendMut::OpenKeyStd.into()).or_else(|_| {
                Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
            })?;

        if err != 0 {
            return Err(crate::io::Error::new(
                crate::io::ErrorKind::Other,
                "error occurred when opening file",
            ));
        }

        let reader = request.reader(*b"KyOR").ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::Other, "invalid response from server")
        })?;

        let fd: u16 = reader.try_get_from().or_else(|_| {
            Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
        })?;

        let len: u64 = reader.try_get_from().or_else(|_| {
            Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
        })?;

        Ok(File { len, fd })
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        Ok(FileAttr { kind: FileType::Key, len: self.len })
    }

    pub fn fsync(&self) -> io::Result<()> {
        // println!("rust: File::fsync()");
        unsupported()
    }

    pub fn datasync(&self) -> io::Result<()> {
        // println!("rust: File::datasync()");
        unsupported()
    }

    pub fn truncate(&self, _size: u64) -> io::Result<()> {
        // println!("rust: File::truncate()");
        unsupported()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        #[repr(C, align(4096))]
        struct ReadBuffer {
            data: [u8; 4096],
        }
        let mut buffer = ReadBuffer { data: [0u8; 4096] };
        let buffer_len = buffer.data.len();

        let (offset, valid) = lend_mut(
            pddb_server(),
            PddbLendMut::ReadKeyStd(self.fd).into(),
            &mut buffer.data,
            0,
            buf.len().min(buffer_len),
        )
        .map_err(|_| {
            crate::io::Error::new(crate::io::ErrorKind::Other, "read() encountered an error")
        })?;

        if offset != 0 {
            return Err(crate::io::Error::new(
                crate::io::ErrorKind::Other,
                "read() encountered an error",
            ));
        }
        let valid = buf.len().min(valid).min(buffer.data.len());
        let contents = &buffer.data[0..valid];
        for (src, dest) in contents.iter().zip(buf.iter_mut()) {
            *dest = *src;
        }
        Ok(valid)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|buf| self.read(buf), bufs)
    }

    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        crate::io::default_read_buf(|buf| self.read(buf), cursor)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        #[repr(C, align(4096))]
        struct ReadBuffer {
            data: [u8; 4096],
        }
        let mut buffer = ReadBuffer { data: [0u8; 4096] };

        let valid = buf.len().min(buffer.data.len());
        {
            let contents = &mut buffer.data[0..valid];
            for (src, dest) in buf.iter().zip(contents.iter_mut()) {
                *dest = *src;
            }
        }

        // This needs to be mutable for now because pddb uses libxous which doesn't
        // support returning values with non-mutable lends, and we need to get the
        // "offset" as a return value.
        let (offset, valid) = lend_mut(
            pddb_server(),
            PddbLend::WriteKeyStd(self.fd).into(),
            &mut buffer.data,
            0,
            valid,
        )
        .map_err(|_| {
            crate::io::Error::new(crate::io::ErrorKind::Other, "write() encountered an error")
        })?;

        if offset == 0 {
            Ok(valid)
        } else {
            Err(crate::io::Error::new(
                crate::io::ErrorKind::Other,
                "write operation encountered an error",
            ))
        }
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|buf| self.write(buf), bufs)
    }

    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn flush(&self) -> io::Result<()> {
        // println!("rust: File::flush()");
        unsupported()
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        let result =
            blocking_scalar(pddb_server(), PddbBlockingScalar::SeekKeyStd(self.fd, pos).into())
                .map_err(|_| {
                    crate::io::Error::new(crate::io::ErrorKind::NotSeekable, "error when seeking")
                })?;

        Ok((result[0] as u64) | ((result[1] as u64) << 32))
    }

    pub fn duplicate(&self) -> io::Result<File> {
        unsupported()
    }

    pub fn set_permissions(&self, _perm: FilePermissions) -> io::Result<()> {
        unsupported()
    }

    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        unsupported()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        blocking_scalar(pddb_server(), PddbBlockingScalar::CloseKeyStd(self.fd).into()).unwrap();
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder {}
    }

    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        let path_as_str = p.as_os_str().to_str().ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;

        let mut request = super::senres::Stack::<4096>::new();

        let mut writer = request.writer(*b"NuDQ").ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;
        writer.append(path_as_str);

        // Make the actual call
        request.lend_mut(pddb_server(), PddbLendMut::CreateDictStd.into()).or_else(|_| {
            Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
        })?;
        Ok(())
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File").field("fd", &self.fd).finish()
    }
}

pub fn readdir(p: &Path) -> io::Result<ReadDir> {
    let path_as_str = p.as_os_str().to_str().ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
    })?;
    let (_basis, _dict) = match crate::sys::path::split_basis_and_dict(path_as_str, || None) {
        Ok(s) => s,
        Err(_) => {
            return Err(crate::io::Error::new(crate::io::ErrorKind::Other, "path was not valid"));
        }
    };

    let mut request = super::senres::Stack::<4096>::new();

    // Write the request to the call
    {
        let mut writer = request.writer(*b"PthQ").ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;
        writer.append(path_as_str);
    }

    // Make the actual call
    request.lend_mut(pddb_server(), PddbLendMut::ListPathStd.into()).or_else(|_| {
        Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
    })?;

    // Read the data back
    let reader = request.reader(*b"PthR").ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::Other, "invalid response from server")
    })?;

    let mut entries = vec![];
    let count = reader.try_get_from::<u32>().unwrap() as usize;
    for _ in 0..count {
        let name = reader.try_get_ref_from().unwrap();
        let kind = match reader.try_get_from::<u8>() {
            Ok(0) => FileType::Basis,
            Ok(1) => FileType::Dict,
            Ok(2) => FileType::Key,
            Ok(3) => FileType::DictKey,
            Ok(4) => FileType::None,
            _ => FileType::Unknown,
        };
        let mut path = path_as_str.to_owned();
        if !path.is_empty() && !path.ends_with(crate::path::MAIN_SEPARATOR) {
            path.push(crate::path::MAIN_SEPARATOR);
        }
        path.push_str(name);
        entries.push(DirEntry {
            name: name.to_owned(),
            path,
            // basis: basis.map(|m| m.to_owned()),
            kind,
        });
    }

    return Ok(ReadDir { entries, root: p.to_owned() });
}

pub fn unlink(p: &Path) -> io::Result<()> {
    let path_as_str = p.as_os_str().to_str().ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
    })?;
    let mut request = super::senres::Stack::<4096>::new();
    {
        let mut writer = request.writer(*b"RmKQ").ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;
        writer.append(path_as_str);
    }

    // Make the actual call
    let (err, _) =
        request.lend_mut(pddb_server(), PddbLendMut::DeleteKeyStd.into()).or_else(|_| {
            Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
        })?;

    if err != 0 {
        return Err(crate::io::Error::new(crate::io::ErrorKind::Other, "error during operation"));
    }
    Ok(())
}

pub fn rename(_old: &Path, _new: &Path) -> io::Result<()> {
    // println!("rust: rename()");
    unsupported()
}

pub fn set_perm(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    // println!("rust: set_perm()");
    unsupported()
}

pub fn rmdir(p: &Path) -> io::Result<()> {
    let path_as_str = p.as_os_str().to_str().ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
    })?;

    let mut request = super::senres::Stack::<4096>::new();

    let mut writer = request.writer(*b"RmDQ").ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
    })?;
    writer.append(path_as_str);

    // Make the actual call
    let (err, _) =
        request.lend_mut(pddb_server(), PddbLendMut::DeleteDictStd.into()).or_else(|_| {
            Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
        })?;
    if err != 0 {
        return Err(crate::io::Error::new(crate::io::ErrorKind::Other, "error during operation"));
    }
    Ok(())
}

pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    for child in readdir(path)? {
        let child = child?;
        let child_type = child.file_type()?;
        if child_type.is_dir() {
            remove_dir_all(&child.path())?;
        } else {
            unlink(&child.path())?;
        }
    }
    rmdir(path)
}

pub fn readlink(p: &Path) -> io::Result<PathBuf> {
    stat(p)?;
    Err(io::const_io_error!(io::ErrorKind::InvalidInput, "not a symbolic link"))
}

pub fn symlink(_original: &Path, _link: &Path) -> io::Result<()> {
    // This target doesn't support symlinks
    unsupported()
}

pub fn link(_src: &Path, _dst: &Path) -> io::Result<()> {
    // This target doesn't support links
    unsupported()
}

pub fn stat(p: &Path) -> io::Result<FileAttr> {
    let path_as_str = p.as_os_str().to_str().ok_or_else(|| {
        crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
    })?;
    let mut request = super::senres::Stack::<4096>::new();

    // Write the request to the call
    {
        let mut writer = request.writer(*b"StaQ").ok_or_else(|| {
            crate::io::Error::new(crate::io::ErrorKind::InvalidFilename, "invalid path")
        })?;
        writer.append(path_as_str);
    }

    // Make the actual call
    request.lend_mut(pddb_server(), PddbLendMut::StatPathStd.into()).or_else(|_| {
        Err(crate::io::Error::new(crate::io::ErrorKind::Other, "unable to query database"))
    })?;

    // Read the data back
    let reader = request.reader(*b"StaR").expect("unable to get reader");
    let kind = match reader.try_get_from::<u8>() {
        Ok(0) => FileType::Basis,
        Ok(1) => FileType::Dict,
        Ok(2) => FileType::Key,
        Ok(3) => FileType::DictKey,
        Ok(4) => FileType::None,
        Ok(5) => FileType::Unknown,
        _ => FileType::Unknown,
    };

    match kind {
        FileType::None | FileType::Unknown => Err(crate::io::Error::new(
            crate::io::ErrorKind::NotFound,
            "File or directory does not exist, or is corrupted",
        )),
        _ => Ok(FileAttr { kind, len: 0 }),
    }
}

pub fn lstat(p: &Path) -> io::Result<FileAttr> {
    // This target doesn't support symlinks
    stat(p)
}

pub fn canonicalize(_p: &Path) -> io::Result<PathBuf> {
    // println!("rust: canonicalize()");
    unsupported()
}

pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    use crate::fs::File;

    let mut reader = File::open(from)?;
    let mut writer = File::create(to)?;

    io::copy(&mut reader, &mut writer)
}
