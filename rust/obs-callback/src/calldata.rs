//! Safe calldata stack.
//!
//! The byte layout matches `libobs/callback/calldata.c.h`. Odd cases pinned
//! by `test/cmocka/test_calldata.c` (exact-fit growth, a null string stored
//! as size 0, a get that checks size rather than type) are characterized,
//! not endorsed. `bnum_allocs` stays with that C test: this core keeps an
//! owned buffer, and the shim is what calls `bmalloc`.

use std::mem::size_of;

const USZ: usize = size_of::<usize>();
/// First owned allocation. A smaller parameter block is still reported as
/// its real used size.
const FIRST_ALLOC: usize = 128;

/// `blog` text for a fixed stack that cannot grow. The shim logs this at
/// `LOG_ERROR` and does not store the write.
pub const FIXED_OVERFLOW_MESSAGE: &str = "Tried to go above fixed calldata stack size!";

/// Result of [`Calldata::get_string`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringGet<'a> {
    /// No parameter with that name. C leaves the out pointer unchanged.
    Missing,
    /// Stored as a zero-size parameter. C returns true and a null pointer.
    Null,
    /// Bytes in the stack, including the trailing NUL.
    Data(&'a [u8]),
}

enum Storage<'a> {
    Empty,
    Owned(Vec<u8>),
    Fixed(&'a mut [u8]),
}

enum Find {
    /// `size == 0`. C does not walk the stack.
    Empty,
    /// Index of the parameter's data-size field.
    Found(usize),
    /// Index of the terminating 0.
    End(usize),
    /// The size fields walk off the used bytes. API-built stacks do not.
    Corrupt,
}

/// Procedure-call parameter stack.
///
/// An owned stack grows. A fixed stack borrows a caller buffer and refuses
/// a write that does not fit.
pub struct Calldata<'a> {
    storage: Storage<'a>,
    size: usize,
    capacity: usize,
    fixed: bool,
    overflows: u32,
    /// The size the last refused write needed.
    wanted: usize,
}

impl Default for Calldata<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl Calldata<'static> {
    /// Empty stack. Nothing is allocated until the first stored parameter.
    pub fn new() -> Self {
        Self {
            storage: Storage::Empty,
            size: 0,
            capacity: 0,
            fixed: false,
            overflows: 0,
            wanted: 0,
        }
    }

    /// Owned copy of an existing stack. `bytes` is the full capacity.
    /// A null C stack uses [`Self::new`] instead: an empty copy is not the
    /// first-allocation path.
    pub(crate) fn from_copy(bytes: &[u8], size: usize, fixed: bool) -> Self {
        let capacity = bytes.len();
        Self {
            storage: Storage::Owned(bytes.to_vec()),
            size,
            capacity,
            fixed,
            overflows: 0,
            wanted: 0,
        }
    }
}

impl<'a> Calldata<'a> {
    /// An existing stack, edited in place (#1749): `bytes` is the whole
    /// allocation and `size` the used part. It never grows. A write that
    /// does not fit counts as an overflow and leaves the bytes as they were,
    /// so the caller can retry it on a copy that grows.
    pub(crate) fn in_place(bytes: &'a mut [u8], size: usize) -> Self {
        let capacity = bytes.len();
        Self {
            storage: Storage::Fixed(bytes),
            size,
            capacity,
            fixed: true,
            overflows: 0,
            wanted: 0,
        }
    }

    /// Borrow `buf` as a fixed stack. The first `size_t` is cleared.
    pub fn fixed(buf: &'a mut [u8]) -> Self {
        let capacity = buf.len();
        let mut cd = Self {
            storage: Storage::Fixed(buf),
            size: 0,
            capacity,
            fixed: true,
            overflows: 0,
            wanted: 0,
        };
        cd.clear();
        cd
    }

    pub fn clear(&mut self) {
        if matches!(self.storage, Storage::Empty) {
            return;
        }
        self.size = USZ;
        let buf = self.bytes_mut();
        if buf.len() >= USZ {
            write_usize(buf, 0, 0);
        }
    }

    /// Release an owned stack. A fixed stack is left untouched.
    pub fn free(&mut self) {
        if !self.fixed {
            self.storage = Storage::Empty;
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_fixed(&self) -> bool {
        self.fixed
    }

    /// The whole allocation, or `None` when nothing has been allocated.
    /// Bytes past [`Self::size`] are not defined.
    pub fn stack(&self) -> Option<&[u8]> {
        match &self.storage {
            Storage::Empty => None,
            Storage::Owned(bytes) => Some(bytes.as_slice()),
            Storage::Fixed(bytes) => Some(bytes),
        }
    }

    /// How many fixed writes were refused. The shim logs each one.
    pub fn overflow_count(&self) -> u32 {
        self.overflows
    }

    /// The size the last refused write needed: a heap stack edited
    /// [`Self::in_place`] grows to it (or to double the capacity) and
    /// retries, as `cd_ensure_capacity` does.
    pub(crate) fn wanted(&self) -> usize {
        self.wanted
    }

    pub fn set_data(&mut self, name: &str, data: &[u8]) {
        self.set_bytes(c_name(name), data);
    }

    pub(crate) fn set_bytes(&mut self, name: &[u8], data: &[u8]) {
        let name = c_name_bytes(name);
        if name.is_empty() {
            return;
        }
        if !self.fixed && matches!(self.storage, Storage::Empty) {
            self.set_first(name, data);
            return;
        }
        match find_param(self.bytes(), self.size, name) {
            Find::Found(pos) => self.replace_param(pos, data),
            Find::End(pos) => self.append_param(pos, name, data),
            // `size == 0` with a live stack is undefined in C (the cursor
            // is never written). API-built stacks are not in that state.
            Find::Empty | Find::Corrupt => {}
        }
    }

    /// `Some` only when `name` is stored with exactly `size` bytes.
    pub fn get_data(&self, name: &str, size: usize) -> Option<&[u8]> {
        self.get_bytes(c_name(name), size)
    }

    pub(crate) fn get_bytes(&self, name: &[u8], size: usize) -> Option<&[u8]> {
        get_in(self.bytes(), self.size, name, size)
    }

    pub fn get_string(&self, name: &str) -> StringGet<'_> {
        self.string_at(c_name(name))
    }

    pub(crate) fn string_at(&self, name: &[u8]) -> StringGet<'_> {
        string_in(self.bytes(), self.size, name)
    }

    /// `None` for a missing name and for a null string. `Some("")` for an
    /// empty C string. Non-UTF-8 bytes are `None`; [`Self::get_string`]
    /// still returns them.
    pub fn string(&self, name: &str) -> Option<&str> {
        match self.get_string(name) {
            StringGet::Data(bytes) => {
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                std::str::from_utf8(&bytes[..end]).ok()
            }
            StringGet::Missing | StringGet::Null => None,
        }
    }

    pub fn set_int(&mut self, name: &str, value: i64) {
        self.set_data(name, &value.to_ne_bytes());
    }

    pub fn get_int(&self, name: &str) -> Option<i64> {
        let bytes = self.get_data(name, size_of::<i64>())?;
        Some(i64::from_ne_bytes(bytes.try_into().ok()?))
    }

    pub fn int(&self, name: &str) -> i64 {
        self.get_int(name).unwrap_or(0)
    }

    pub fn set_float(&mut self, name: &str, value: f64) {
        self.set_data(name, &value.to_ne_bytes());
    }

    pub fn get_float(&self, name: &str) -> Option<f64> {
        let bytes = self.get_data(name, size_of::<f64>())?;
        Some(f64::from_ne_bytes(bytes.try_into().ok()?))
    }

    pub fn float(&self, name: &str) -> f64 {
        self.get_float(name).unwrap_or(0.0)
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set_data(name, &[u8::from(value)]);
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        let bytes = self.get_data(name, 1)?;
        Some(bytes[0] != 0)
    }

    pub fn bool(&self, name: &str) -> bool {
        self.get_bool(name).unwrap_or(false)
    }

    pub fn set_ptr(&mut self, name: &str, value: usize) {
        self.set_data(name, &value.to_ne_bytes());
    }

    pub fn get_ptr(&self, name: &str) -> Option<usize> {
        let bytes = self.get_data(name, size_of::<usize>())?;
        Some(usize::from_ne_bytes(bytes.try_into().ok()?))
    }

    /// `0` when the name is missing. A stored null pointer is also `0`.
    pub fn ptr(&self, name: &str) -> usize {
        self.get_ptr(name).unwrap_or(0)
    }

    /// `None` stores a zero-size parameter. `Some` stores the characters
    /// plus a trailing NUL.
    pub fn set_string(&mut self, name: &str, value: Option<&str>) {
        match value {
            Some(text) => {
                let mut bytes = Vec::with_capacity(text.len() + 1);
                bytes.extend_from_slice(text.as_bytes());
                bytes.push(0);
                self.set_data(name, &bytes);
            }
            None => self.set_data(name, &[]),
        }
    }

    fn set_first(&mut self, name: &[u8], data: &[u8]) {
        let (used, cap) = first_param(name, data);
        let mut buf = vec![0u8; cap];
        write_param(&mut buf, 0, name, data);
        self.size = used;
        self.capacity = cap;
        self.fixed = false;
        self.storage = Storage::Owned(buf);
    }

    fn replace_param(&mut self, pos: usize, data: &[u8]) {
        let cur_size = {
            let bytes = self.bytes();
            match read_usize_within(bytes, pos, self.size) {
                Some(size) => size,
                None => return,
            }
        };
        let new_size = data.len();
        if cur_size < new_size {
            let offset = new_size - cur_size;
            let Some(needed) = self.size.checked_add(offset) else {
                return;
            };
            if !self.ensure(needed) {
                return;
            }
            let old = self.size;
            if pos > old {
                return;
            }
            let tail = old - pos;
            self.bytes_mut().copy_within(pos..pos + tail, pos + offset);
            self.size = old + offset;
        } else if cur_size > new_size {
            let offset = cur_size - new_size;
            if offset > self.size || pos > self.size - offset {
                return;
            }
            let new_total = self.size - offset;
            let tail = new_total - pos;
            self.bytes_mut()
                .copy_within(pos + offset..pos + offset + tail, pos);
            self.size = new_total;
        }
        self.write_data(pos, data);
    }

    fn append_param(&mut self, pos: usize, name: &[u8], data: &[u8]) {
        let name_len = name.len() + 1;
        let Some(add) = name_len
            .checked_add(data.len())
            .and_then(|n| n.checked_add(USZ * 2))
        else {
            return;
        };
        let Some(needed) = self.size.checked_add(add) else {
            return;
        };
        if !self.ensure(needed) {
            return;
        }
        self.size += add;
        let buf = self.bytes_mut();
        write_named(buf, pos, name, data);
    }

    fn write_data(&mut self, pos: usize, data: &[u8]) {
        let buf = self.bytes_mut();
        if !write_usize(buf, pos, data.len()) {
            return;
        }
        if data.is_empty() {
            return;
        }
        let start = pos + USZ;
        let Some(end) = start.checked_add(data.len()) else {
            return;
        };
        if let Some(slot) = buf.get_mut(start..end) {
            slot.copy_from_slice(data);
        }
    }

    /// `cd_ensure_capacity`: grow when `new_size > capacity`. Fixed stacks
    /// refuse instead of allocating.
    fn ensure(&mut self, new_size: usize) -> bool {
        if new_size <= self.capacity {
            return true;
        }
        if self.fixed {
            self.overflows += 1;
            self.wanted = new_size;
            return false;
        }
        let mut new_cap = self.capacity.saturating_mul(2);
        if new_cap < new_size {
            new_cap = new_size;
        }
        match &mut self.storage {
            Storage::Owned(bytes) => bytes.resize(new_cap, 0),
            Storage::Empty => self.storage = Storage::Owned(vec![0u8; new_cap]),
            Storage::Fixed(_) => return false,
        }
        self.capacity = new_cap;
        true
    }

    fn bytes(&self) -> &[u8] {
        match &self.storage {
            Storage::Empty => &[],
            Storage::Owned(bytes) => bytes.as_slice(),
            Storage::Fixed(bytes) => bytes,
        }
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        match &mut self.storage {
            Storage::Empty => &mut [],
            Storage::Owned(bytes) => bytes.as_mut_slice(),
            Storage::Fixed(bytes) => bytes,
        }
    }
}

/// The used size and the capacity of a new stack whose first parameter is
/// `name` (already cut at its NUL, not empty) with `data`.
pub(crate) fn first_param(name: &[u8], data: &[u8]) -> (usize, usize) {
    let used = USZ * 3 + name.len() + 1 + data.len();
    (used, used.max(FIRST_ALLOC))
}

/// Writes the first parameter of a new stack into `buf`, which has the
/// capacity [`first_param`] gave. `name` is not empty and has no NUL, as
/// the shim's names are.
pub(crate) fn write_first(buf: &mut [u8], name: &[u8], data: &[u8]) {
    debug_assert!(!name.is_empty() && !name.contains(&0));
    write_param(buf, 0, name, data);
}

/// `calldata_get_data` on a stack's bytes (the whole allocation) and its
/// used size. The shim reads the C buffer with it in place (#1749).
pub(crate) fn get_in<'b>(
    bytes: &'b [u8],
    size: usize,
    name: &[u8],
    data_size: usize,
) -> Option<&'b [u8]> {
    let name = c_name_bytes(name);
    if name.is_empty() {
        return None;
    }
    let Find::Found(pos) = find_param(bytes, size, name) else {
        return None;
    };
    let stored = read_usize_within(bytes, pos, size)?;
    if stored != data_size {
        return None;
    }
    let start = pos + USZ;
    let end = start.checked_add(stored)?;
    if end > size {
        return None;
    }
    bytes.get(start..end)
}

/// `calldata_get_string` on a stack's bytes and its used size, as
/// [`get_in`].
pub(crate) fn string_in<'b>(bytes: &'b [u8], size: usize, name: &[u8]) -> StringGet<'b> {
    let name = c_name_bytes(name);
    if name.is_empty() {
        return StringGet::Missing;
    }
    let Find::Found(pos) = find_param(bytes, size, name) else {
        return StringGet::Missing;
    };
    let Some(data_size) = read_usize_within(bytes, pos, size) else {
        return StringGet::Missing;
    };
    if data_size == 0 {
        return StringGet::Null;
    }
    let start = pos + USZ;
    let Some(end) = start.checked_add(data_size) else {
        return StringGet::Missing;
    };
    if end > size {
        return StringGet::Missing;
    }
    match bytes.get(start..end) {
        Some(data) => StringGet::Data(data),
        None => StringGet::Missing,
    }
}

/// Bytes of `name` up to the first NUL. C measures the name with `strlen`.
fn c_name(name: &str) -> &[u8] {
    c_name_bytes(name.as_bytes())
}

fn c_name_bytes(name: &[u8]) -> &[u8] {
    match name.iter().position(|b| *b == 0) {
        Some(i) => &name[..i],
        None => name,
    }
}

/// `strcmp` of a stored name with `name` (which has no NUL): the stored
/// bytes start with `name` and a NUL follows. This is the same as cutting
/// the stored name at its first NUL, without the scan.
fn name_eq(stored: &[u8], name: &[u8]) -> bool {
    stored.get(name.len()) == Some(&0) && stored.starts_with(name)
}

fn find_param(bytes: &[u8], size: usize, name: &[u8]) -> Find {
    if size == 0 {
        return Find::Empty;
    }
    if size > bytes.len() {
        return Find::Corrupt;
    }
    let Some(mut name_size) = read_usize_within(bytes, 0, size) else {
        return Find::Corrupt;
    };
    let mut pos = USZ;
    while name_size != 0 {
        let Some(name_end) = pos.checked_add(name_size) else {
            return Find::Corrupt;
        };
        if name_end > size {
            return Find::Corrupt;
        }
        let matched = name_eq(&bytes[pos..name_end], name);
        pos = name_end;
        if matched {
            return Find::Found(pos);
        }
        let Some(param_size) = read_usize_within(bytes, pos, size) else {
            return Find::Corrupt;
        };
        pos += USZ;
        let Some(next) = pos.checked_add(param_size) else {
            return Find::Corrupt;
        };
        if next > size {
            return Find::Corrupt;
        }
        pos = next;
        let Some(next_name) = read_usize_within(bytes, pos, size) else {
            return Find::Corrupt;
        };
        name_size = next_name;
        pos += USZ;
    }
    if pos < USZ {
        return Find::Corrupt;
    }
    Find::End(pos - USZ)
}

fn write_param(buf: &mut [u8], pos: usize, name: &[u8], data: &[u8]) {
    write_named(buf, pos, name, data);
}

fn write_named(buf: &mut [u8], mut pos: usize, name: &[u8], data: &[u8]) {
    let name_len = name.len() + 1;
    if !write_usize(buf, pos, name_len) {
        return;
    }
    pos += USZ;
    let Some(name_end) = pos.checked_add(name_len) else {
        return;
    };
    if let Some(slot) = buf.get_mut(pos..name_end) {
        slot[..name.len()].copy_from_slice(name);
        slot[name.len()] = 0;
    } else {
        return;
    }
    pos = name_end;
    if !write_usize(buf, pos, data.len()) {
        return;
    }
    pos += USZ;
    if !data.is_empty() {
        let Some(data_end) = pos.checked_add(data.len()) else {
            return;
        };
        if let Some(slot) = buf.get_mut(pos..data_end) {
            slot.copy_from_slice(data);
        } else {
            return;
        }
        pos = data_end;
    }
    write_usize(buf, pos, 0);
}

fn read_usize(bytes: &[u8], at: usize) -> Option<usize> {
    let end = at.checked_add(USZ)?;
    let slot = bytes.get(at..end)?;
    if USZ == 8 {
        let mut tmp = [0u8; 8];
        tmp.copy_from_slice(slot);
        Some(u64::from_ne_bytes(tmp) as usize)
    } else if USZ == 4 {
        let mut tmp = [0u8; 4];
        tmp.copy_from_slice(slot);
        Some(u32::from_ne_bytes(tmp) as usize)
    } else {
        None
    }
}

fn read_usize_within(bytes: &[u8], at: usize, limit: usize) -> Option<usize> {
    let end = at.checked_add(USZ)?;
    if end > limit {
        return None;
    }
    read_usize(bytes, at)
}

fn write_usize(bytes: &mut [u8], at: usize, value: usize) -> bool {
    let Some(end) = at.checked_add(USZ) else {
        return false;
    };
    let Some(slot) = bytes.get_mut(at..end) else {
        return false;
    };
    if USZ == 8 {
        slot.copy_from_slice(&(value as u64).to_ne_bytes());
        true
    } else if USZ == 4 {
        slot.copy_from_slice(&(value as u32).to_ne_bytes());
        true
    } else {
        false
    }
}
