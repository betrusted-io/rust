///! Senres is a mechanism for serializing data across FFI boundaries. It allows
///! for constructing rich structs. Senres has no external dependencies and
///! ensures that allocated memory is aligned for the target platform.
///!
///! Senres messages have a magic number as well as a per-message signature so
///! as to ensure a given message isn't misinterpreted as a different struct.
use core::cell::Cell;
use core::convert::TryInto;

#[cfg(test)]
pub(crate) mod tests;

/// Senres V1 always begins with the number 0x344cb6ca to indicate it's valid.
/// This number will change on subsequent versions.
const SENRES_V1_MAGIC: u32 = 0x344cb6ca;

/// A struct to send and receive data. This struct must be page-aligned
/// in order to be sendable across processes.
#[repr(C, align(4096))]
pub struct Stack<const N: usize = 4096> {
    data: [u8; N],
}

pub trait Senres {
    fn as_slice(&self) -> &[u8];
    fn as_ptr(&self) -> *const u8;
    fn len(&self) -> usize;

    fn can_create_writer(&self) -> bool {
        true
    }

    fn reader(&self, fourcc: [u8; 4]) -> Option<Reader<Self>>
    where
        Self: core::marker::Sized,
    {
        let reader = Reader { backing: self, offset: core::cell::Cell::new(0) };
        if SENRES_V1_MAGIC != reader.try_get_from().ok()? {
            return None;
        }
        let target_fourcc: [u8; 4] = reader.try_get_from().ok()?;
        if target_fourcc != fourcc {
            return None;
        }
        Some(reader)
    }
}

pub trait SenresMut: Senres {
    fn as_mut_slice(&mut self) -> &mut [u8];
    fn as_mut_ptr(&mut self) -> *mut u8;
    fn writer(&mut self, fourcc: [u8; 4]) -> Option<Writer<Self>>
    where
        Self: core::marker::Sized,
    {
        if !self.can_create_writer() {
            return None;
        }
        let mut writer = Writer { backing: self, offset: 0 };
        writer.append(SENRES_V1_MAGIC);
        writer.append(fourcc);
        Some(writer)
    }
}

pub struct Writer<'a, Backing: SenresMut> {
    backing: &'a mut Backing,
    offset: usize,
}

pub struct DelayedWriter<Backing: SenresMut, T: SenSer<Backing>> {
    offset: usize,
    _kind: core::marker::PhantomData<T>,
    _backing: core::marker::PhantomData<Backing>,
}

pub struct Reader<'a, Backing: Senres> {
    backing: &'a Backing,
    offset: Cell<usize>,
}

pub trait SenSer<Backing: SenresMut> {
    fn append_to(&self, senres: &mut Writer<Backing>);
}

pub trait RecDes<Backing: Senres> {
    fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()>
    where
        Self: core::marker::Sized;
}

pub trait RecDesRef<'a, Backing: Senres> {
    fn try_get_ref_from(senres: &'a Reader<Backing>) -> Result<&'a Self, ()>;
}

impl<const N: usize> Stack<N> {
    /// Ensure that `N` is a multiple of 4096. This constant should
    /// be evaluated in the constructor function.
    const CHECK_ALIGNED: () = if N & 4095 != 0 {
        panic!("Senres size must be a multiple of 4096")
    };

    pub const fn new() -> Self {
        // Ensure the `N` that was specified is a multiple of 4096
        #[allow(clippy::no_effect, clippy::let_unit_value)]
        let _ = Self::CHECK_ALIGNED;
        Stack { data: [0u8; N] }
    }
}

impl<const N: usize> SenresMut for Stack<N> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        &mut self.data as *mut _ as *mut u8
    }
}

impl<const N: usize> Senres for Stack<N> {
    fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }
    fn len(&self) -> usize {
        N
    }
    fn as_ptr(&self) -> *const u8 {
        &self.data as *const _ as *const u8
    }
}

impl<'a, Backing: SenresMut> Writer<'a, Backing> {
    pub fn append<T: SenSer<Backing>>(&mut self, other: T) {
        other.append_to(self);
    }

    pub fn delayed_append<T: SenSer<Backing>>(&mut self) -> DelayedWriter<Backing, T> {
        let delayed_writer = DelayedWriter {
            offset: self.offset,
            _backing: core::marker::PhantomData::<Backing>,
            _kind: core::marker::PhantomData::<T>,
        };
        self.offset += core::mem::size_of::<T>();
        delayed_writer
    }

    pub fn do_delayed_append<T: SenSer<Backing>>(
        &mut self,
        delayed_writer: DelayedWriter<Backing, T>,
        other: T,
    ) {
        let current_offset = self.offset;
        self.offset = delayed_writer.offset;
        other.append_to(self);
        if self.offset != delayed_writer.offset + core::mem::size_of::<T>() {
            panic!("writer incorrectly increased offset");
        }
        self.offset = current_offset;
    }

    pub fn align_to(&mut self, alignment: usize) {
        while self.offset & (alignment - 1) != 0 {
            self.offset += 1;
        }
    }
}

impl<'a, Backing: Senres> Reader<'a, Backing> {
    pub fn try_get_from<T: RecDes<Backing>>(&self) -> Result<T, ()> {
        T::try_get_from(self)
    }

    pub fn try_get_ref_from<T: RecDesRef<'a, Backing> + ?Sized>(&'a self) -> Result<&T, ()> {
        T::try_get_ref_from(self)
    }

    fn align_to(&self, alignment: usize) {
        while self.offset.get() & (alignment - 1) != 0 {
            self.offset.set(self.offset.get() + 1);
        }
    }
}

macro_rules! primitive_impl {
    ($SelfT:ty) => {
        impl<Backing: SenresMut> SenSer<Backing> for $SelfT {
            fn append_to(&self, senres: &mut Writer<Backing>) {
                senres.align_to(core::mem::align_of::<Self>());
                for (src, dest) in self
                    .to_le_bytes()
                    .iter()
                    .zip(senres.backing.as_mut_slice()[senres.offset..].iter_mut())
                {
                    *dest = *src;
                    senres.offset += 1;
                }
            }
        }

        impl<Backing: Senres> RecDes<Backing> for $SelfT {
            fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()> {
                senres.align_to(core::mem::align_of::<Self>());
                let my_size = core::mem::size_of::<Self>();
                let offset = senres.offset.get();
                if offset + my_size > senres.backing.as_slice().len() {
                    return Err(());
                }
                let val = Self::from_le_bytes(
                    senres.backing.as_slice()[offset..offset + my_size].try_into().unwrap(),
                );
                senres.offset.set(offset + my_size);
                Ok(val)
            }
        }
    };
}

impl<Backing: SenresMut> SenSer<Backing> for bool {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        senres.align_to(core::mem::align_of::<Self>());
        senres.backing.as_mut_slice()[senres.offset] = if *self { 1 } else { 0 };
        senres.offset += 1;
    }
}

impl<Backing: Senres> RecDes<Backing> for bool {
    fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()> {
        senres.align_to(core::mem::align_of::<Self>());
        let my_size = core::mem::size_of::<Self>();
        let offset = senres.offset.get();
        if offset + my_size > senres.backing.as_slice().len() {
            return Err(());
        }
        let val = match senres.backing.as_slice()[offset] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(()),
        };
        senres.offset.set(offset + my_size);
        val
    }
}

impl<T: SenSer<Backing>, Backing: SenresMut> SenSer<Backing> for Option<T> {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        if let Some(val) = self {
            senres.append(1u8);
            val.append_to(senres);
        } else {
            senres.append(0u8);
        }
    }
}

impl<T: RecDes<Backing>, Backing: Senres> RecDes<Backing> for Option<T> {
    fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()> {
        if senres.offset.get() + 1 > senres.backing.as_slice().len() {
            return Err(());
        }
        let check = senres.try_get_from::<u8>()?;
        if check == 0 {
            return Ok(None);
        }
        if check != 1 {
            return Err(());
        }
        let my_size = core::mem::size_of::<Self>();
        if senres.offset.get() + my_size > senres.backing.as_slice().len() {
            return Err(());
        }
        Ok(Some(T::try_get_from(senres)?))
    }
}

primitive_impl! {u8}
primitive_impl! {i8}
primitive_impl! {u16}
primitive_impl! {i16}
primitive_impl! {u32}
primitive_impl! {i32}
primitive_impl! {u64}
primitive_impl! {i64}

impl<T: SenSer<Backing>, Backing: SenresMut> SenSer<Backing> for &[T] {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        senres.append(self.len() as u32);
        for entry in self.iter() {
            entry.append_to(senres)
        }
    }
}

impl<T: SenSer<Backing>, Backing: SenresMut, const N: usize> SenSer<Backing> for [T; N] {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        // senres.append(self.len() as u32);
        senres.align_to(core::mem::align_of::<Self>());
        for entry in self.iter() {
            entry.append_to(senres)
        }
    }
}

impl<T: RecDes<Backing>, Backing: Senres, const N: usize> RecDes<Backing> for [T; N] {
    fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()> {
        let len = core::mem::size_of::<Self>();
        senres.align_to(core::mem::align_of::<Self>());
        let offset = senres.offset.get();
        if offset + len > senres.backing.as_slice().len() {
            return Err(());
        }

        // See https://github.com/rust-lang/rust/issues/61956 for why this
        // is awful
        let mut output: [core::mem::MaybeUninit<T>; N] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        for elem in &mut output[..] {
            elem.write(T::try_get_from(senres)?);
        }

        // Using &mut as an assertion of unique "ownership"
        let ptr = &mut output as *mut _ as *mut [T; N];
        let res = unsafe { ptr.read() };
        core::mem::forget(output);
        Ok(res)
    }
}

impl<Backing: SenresMut> SenSer<Backing> for str {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        senres.append(self.len() as u32);
        for (src, dest) in
            self.as_bytes().iter().zip(senres.backing.as_mut_slice()[senres.offset..].iter_mut())
        {
            *dest = *src;
            senres.offset += 1;
        }
    }
}

impl<Backing: SenresMut> SenSer<Backing> for &str {
    fn append_to(&self, senres: &mut Writer<Backing>) {
        senres.append(self.len() as u32);
        for (src, dest) in
            self.as_bytes().iter().zip(senres.backing.as_mut_slice()[senres.offset..].iter_mut())
        {
            *dest = *src;
            senres.offset += 1;
        }
    }
}

impl<Backing: Senres> RecDes<Backing> for String {
    fn try_get_from(senres: &Reader<Backing>) -> Result<Self, ()> {
        let len = senres.try_get_from::<u32>()? as usize;
        let offset = senres.offset.get();
        if offset + len > senres.backing.as_slice().len() {
            return Err(());
        }
        core::str::from_utf8(&senres.backing.as_slice()[offset..offset + len]).or(Err(())).map(
            |e| {
                senres.offset.set(offset + len);
                e.to_owned()
            },
        )
    }
}

impl<'a, Backing: Senres> RecDesRef<'a, Backing> for str {
    fn try_get_ref_from(senres: &'a Reader<Backing>) -> Result<&'a Self, ()> {
        let len = senres.try_get_from::<u32>()? as usize;
        let offset = senres.offset.get();
        if offset + len > senres.backing.as_slice().len() {
            return Err(());
        }
        core::str::from_utf8(&senres.backing.as_slice()[offset..offset + len]).or(Err(())).map(
            |e| {
                senres.offset.set(offset + len);
                e
            },
        )
    }
}

impl<'a, Backing: Senres, T: RecDes<Backing>> RecDesRef<'a, Backing> for [T] {
    fn try_get_ref_from(senres: &'a Reader<Backing>) -> Result<&'a Self, ()> {
        let len = senres.try_get_from::<u32>()? as usize;
        let offset = senres.offset.get();
        if offset + (len * core::mem::size_of::<T>()) > senres.backing.as_slice().len() {
            return Err(());
        }
        let ret = unsafe {
            core::slice::from_raw_parts(
                senres.backing.as_slice().as_ptr().add(offset) as *const T,
                len,
            )
        };
        senres.offset.set(offset + len * core::mem::size_of::<T>());
        Ok(ret)
    }
}

impl<const N: usize> Default for Stack<N> {
    fn default() -> Self {
        Self::new()
    }
}
