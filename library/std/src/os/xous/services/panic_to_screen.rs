pub(crate) enum PanicToScreenScalar {
    BeginPanic,
}

impl Into<[usize; 5]> for PanicToScreenScalar {
    fn into(self) -> [usize; 5] {
        match self {
            PanicToScreenScalar::BeginPanic => [1000, 0, 0, 0, 0],
        }
    }
}
