use super::unsupported;
use crate::collections::HashMap;
use crate::error::Error as StdError;
use crate::ffi::{OsStr, OsString};
use crate::fmt;
use crate::io;
use crate::marker::PhantomData;
use crate::os::xous::ffi::Error as XousError;
use crate::path::{self, PathBuf};
use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sync::{Mutex, Once};
use crate::vec;

#[cfg(not(test))]
mod c_compat {
    use crate::os::xous::ffi::exit;
    extern "C" {
        fn main() -> u32;
    }

    #[no_mangle]
    pub extern "C" fn abort() {
        exit(1);
    }

    #[no_mangle]
    pub extern "C" fn _start() {
        exit(unsafe { main() });
    }

    // This function is needed by the panic runtime. The symbol is named in
    // pre-link args for the target specification, so keep that in sync.
    #[no_mangle]
    // NB. used by both libunwind and libpanic_abort
    pub extern "C" fn __rust_abort() -> ! {
        exit(101);
    }
}

pub fn errno() -> i32 {
    0
}

pub fn error_string(errno: i32) -> String {
    Into::<XousError>::into(errno).to_string()
}

pub fn getcwd() -> io::Result<PathBuf> {
    unsupported()
}

pub fn chdir(_: &path::Path) -> io::Result<()> {
    unsupported()
}

pub struct SplitPaths<'a>(!, PhantomData<&'a ()>);

pub fn split_paths(_unparsed: &OsStr) -> SplitPaths<'_> {
    panic!("unsupported")
}

impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        self.0
    }
}

#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(_paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    Err(JoinPathsError)
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "not supported on this platform yet".fmt(f)
    }
}

impl StdError for JoinPathsError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "not supported on this platform yet"
    }
}

pub fn current_exe() -> io::Result<PathBuf> {
    unsupported()
}

static ENV: AtomicUsize = AtomicUsize::new(0);
static ENV_INIT: Once = Once::new();
type EnvStore = Mutex<HashMap<OsString, OsString>>;

fn get_env_store() -> Option<&'static EnvStore> {
    unsafe { (core::ptr::from_exposed_addr::<EnvStore>(ENV.load(Ordering::Relaxed))).as_ref() }
}

fn create_env_store() -> &'static EnvStore {
    ENV_INIT.call_once(|| {
        ENV.store(Box::into_raw(Box::new(EnvStore::default())) as _, Ordering::Relaxed)
    });
    unsafe { &*core::ptr::from_exposed_addr::<EnvStore>(ENV.load(Ordering::Relaxed)) }
}

pub struct Env {
    iter: vec::IntoIter<(OsString, OsString)>,
}

// FIXME(https://github.com/rust-lang/rust/issues/114583): Remove this when <OsStr as Debug>::fmt matches <str as Debug>::fmt.
pub struct EnvStrDebug<'a> {
    slice: &'a [(OsString, OsString)],
}

impl fmt::Debug for EnvStrDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { slice } = self;
        f.debug_list()
            .entries(slice.iter().map(|(a, b)| (a.to_str().unwrap(), b.to_str().unwrap())))
            .finish()
    }
}

impl Env {
    pub fn str_debug(&self) -> impl fmt::Debug + '_ {
        let Self { iter } = self;
        EnvStrDebug { slice: iter.as_slice() }
    }
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { iter } = self;
        f.debug_list().entries(iter.as_slice()).finish()
    }
}

impl !Send for Env {}
impl !Sync for Env {}

impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

pub fn env() -> Env {
    let clone_to_vec = |map: &HashMap<OsString, OsString>| -> Vec<_> {
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };

    let iter = get_env_store()
        .map(|env| clone_to_vec(&env.lock().unwrap()))
        .unwrap_or_default()
        .into_iter();
    Env { iter }
}

pub fn getenv(k: &OsStr) -> Option<OsString> {
    get_env_store().and_then(|s| s.lock().unwrap().get(k).cloned())
}

pub fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    let (k, v) = (k.to_owned(), v.to_owned());
    create_env_store().lock().unwrap().insert(k, v);
    Ok(())
}

pub fn unsetenv(k: &OsStr) -> io::Result<()> {
    if let Some(env) = get_env_store() {
        env.lock().unwrap().remove(k);
    }
    Ok(())
}

pub fn temp_dir() -> PathBuf {
    panic!("no filesystem on this platform")
}

pub fn home_dir() -> Option<PathBuf> {
    None
}

pub fn exit(code: i32) -> ! {
    crate::os::xous::ffi::exit(code as u32);
}

pub fn getpid() -> u32 {
    panic!("no pids on this platform")
}
