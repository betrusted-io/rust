#[repr(usize)]
pub(crate) enum Opcodes {
    // IsMounted = 0,
    // TryMount = 1,

    // WriteKeyFlush = 18,
    // KeyDrop = 20,

    // ListBasisStd = 26,
    // ListDictStd = 28,
    // ListKeyStd = 29,
    OpenKeyStd = 30,
    ReadKeyStd = 31,
    WriteKeyStd = 32,
    CloseKeyStd = 34,
    DeleteKeyStd = 35,
    // LatestBasisStd = 36,
    ListPathStd = 37,
    StatPathStd = 38,
    SeekKeyStd = 39,

    /// Create a dict
    CreateDictStd = 40,

    /// Remove an empty dict
    DeleteDictStd = 41,
}
