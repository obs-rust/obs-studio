//! Safe core of `libobs/obs-data.c`.
//!
//! User, default, and autoselect values share one item. A user getter reads
//! the user value when one is set, otherwise the default, otherwise the
//! autoselect. Default and autoselect getters read only their own slot.
//! JSON without defaults skips an item that has no user value. The
//! with-defaults dump emits whatever the user getter reads.
//!
//! A user value left behind when `set_autoselect` changes the item type is
//! not reinterpreted. Getters for the new type return the empty value.

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use crate::ffi::json;

/// The `RefCell`-style accessors on the per-object lock (#1451).
///
/// libobs shares `obs_data_t`, its items and arrays between threads and
/// makes only the reference count atomic. Each object here is an
/// `Arc<RwLock<_>>`: readers on several threads at once, one writer.
/// Locks are taken in one order, an object before its items, and are never
/// held across a callback or a call into another object's lock of the same
/// kind. A poisoned lock still holds consistent data, because no step here
/// panics with the data half-changed.
trait Lock<T> {
    fn borrow(&self) -> RwLockReadGuard<'_, T>;
    fn borrow_mut(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> Lock<T> for RwLock<T> {
    fn borrow(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| e.into_inner())
    }

    fn borrow_mut(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Null = 0,
    String = 1,
    Number = 2,
    Boolean = 3,
    Object = 4,
    Array = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumType {
    Invalid = 0,
    Int = 1,
    Double = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fps {
    pub numerator: u32,
    pub denominator: u32,
}

/// Result of a frames-per-second read.
///
/// `fps` is `None` when the call does not write the fraction: the value is
/// missing, or the object stores an option string. `option` is set only on
/// that option path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FpsRead {
    pub ok: bool,
    pub fps: Option<Fps>,
    pub option: Option<String>,
}

#[derive(Clone, Copy)]
enum Slot {
    User,
    Default,
    Autoselect,
}

#[derive(Clone)]
enum Value {
    String(String),
    Int(i64),
    Double(f64),
    Bool(bool),
    Object(Option<Data>),
    Array(Option<Array>),
}

impl Value {
    fn ty(&self) -> Type {
        match self {
            Value::String(_) => Type::String,
            Value::Int(_) | Value::Double(_) => Type::Number,
            Value::Bool(_) => Type::Boolean,
            Value::Object(_) => Type::Object,
            Value::Array(_) => Type::Array,
        }
    }

    fn string(val: Option<&str>) -> Self {
        Value::String(val.unwrap_or("").to_string())
    }
}

fn read_int(value: &Value) -> i64 {
    match value {
        Value::Int(v) => *v,
        Value::Double(v) => *v as i64,
        _ => 0,
    }
}

fn read_double(value: &Value) -> f64 {
    match value {
        Value::Double(v) => *v,
        Value::Int(v) => *v as f64,
        _ => 0.0,
    }
}

fn read_bool(value: &Value) -> bool {
    match value {
        Value::Bool(v) => *v,
        _ => false,
    }
}

fn cache_string(slot: &mut Option<CString>, value: &Value) {
    *slot = match value {
        Value::String(text) if !text.is_empty() => CString::new(text.clone()).ok(),
        _ => None,
    };
}

fn c_ptr(slot: &Option<CString>) -> *const std::ffi::c_char {
    slot.as_ref()
        .map(|text| text.as_ptr())
        .unwrap_or(c"".as_ptr())
}

/// Names up to this many bytes are stored in the item's own allocation,
/// as C stores the name in the item block (#1453).
const NAME_INLINE: usize = 31;

/// The hash of a name's bytes, the same for every object in the process.
fn name_hash(bytes: &[u8]) -> u64 {
    static STATE: OnceLock<foldhash::fast::RandomState> = OnceLock::new();
    std::hash::BuildHasher::hash_one(STATE.get_or_init(Default::default), bytes)
}

/// An item name: its bytes, a NUL after them, and their hash. A name with
/// a NUL inside is stored empty, as before.
struct Name {
    hash: u64,
    text: NameText,
}

enum NameText {
    Inline { len: u8, buf: [u8; NAME_INLINE + 1] },
    Heap(Box<CStr>),
}

impl Name {
    fn new(name: &str) -> Self {
        let bytes = if name.as_bytes().contains(&0) {
            &[][..]
        } else {
            name.as_bytes()
        };
        let text = if bytes.len() <= NAME_INLINE {
            let mut buf = [0u8; NAME_INLINE + 1];
            buf[..bytes.len()].copy_from_slice(bytes);
            NameText::Inline {
                len: bytes.len() as u8,
                buf,
            }
        } else {
            NameText::Heap(CString::new(bytes).unwrap_or_default().into_boxed_c_str())
        };
        Self {
            hash: name_hash(bytes),
            text,
        }
    }

    fn bytes(&self) -> &[u8] {
        match &self.text {
            NameText::Inline { len, buf } => &buf[..*len as usize],
            NameText::Heap(text) => text.to_bytes(),
        }
    }

    fn is(&self, hash: u64, bytes: &[u8]) -> bool {
        self.hash == hash && self.bytes() == bytes
    }

    fn as_c_str(&self) -> &CStr {
        match &self.text {
            // `buf` is zero after `len` bytes that hold no NUL.
            NameText::Inline { len, buf } => {
                CStr::from_bytes_with_nul(&buf[..=*len as usize]).unwrap_or(c"")
            }
            NameText::Heap(text) => text,
        }
    }

    fn string(&self) -> String {
        String::from_utf8_lossy(self.bytes()).into_owned()
    }
}

/// An item and its name. The name never changes, so it is outside the
/// lock: a lookup reads it without locking the item (#1450). One
/// allocation per item (#1453).
pub(crate) struct ItemNode {
    name: Name,
    lock: RwLock<ItemInner>,
}

impl ItemNode {
    fn new(name: &str, inner: ItemInner) -> Self {
        Self {
            name: Name::new(name),
            lock: RwLock::new(inner),
        }
    }
}

impl std::ops::Deref for ItemNode {
    type Target = RwLock<ItemInner>;

    fn deref(&self) -> &Self::Target {
        &self.lock
    }
}

pub(crate) struct ItemInner {
    ty: Type,
    user: Option<Value>,
    default: Option<Value>,
    autoselect: Option<Value>,
    user_c: Option<CString>,
    default_c: Option<CString>,
    auto_c: Option<CString>,
    parent: Option<Weak<RwLock<DataInner>>>,
}

impl ItemInner {
    fn create(slot: Slot, value: Value, parent: Weak<RwLock<DataInner>>) -> Self {
        let mut item = Self {
            ty: value.ty(),
            user: None,
            default: None,
            autoselect: None,
            user_c: None,
            default_c: None,
            auto_c: None,
            parent: Some(parent),
        };
        item.write(slot, value);
        item
    }

    fn write(&mut self, slot: Slot, value: Value) {
        match slot {
            Slot::Default => {
                if (self.user.is_some() || self.default.is_some() || self.autoselect.is_some())
                    && self.ty != value.ty()
                {
                    return;
                }
                self.ty = value.ty();
                cache_string(&mut self.default_c, &value);
                self.default = Some(value);
            }
            Slot::Autoselect => {
                self.ty = value.ty();
                cache_string(&mut self.auto_c, &value);
                self.autoselect = Some(value);
            }
            Slot::User => {
                self.ty = value.ty();
                cache_string(&mut self.user_c, &value);
                self.user = Some(value);
            }
        }
    }

    fn front(&self) -> Option<&Value> {
        self.user
            .as_ref()
            .or(self.default.as_ref())
            .or(self.autoselect.as_ref())
    }

    fn front_c(&self) -> *const std::ffi::c_char {
        if self.user.is_some() {
            c_ptr(&self.user_c)
        } else if self.default.is_some() {
            c_ptr(&self.default_c)
        } else if self.autoselect.is_some() {
            c_ptr(&self.auto_c)
        } else {
            c"".as_ptr()
        }
    }

    fn user_string(&self) -> String {
        if self.ty != Type::String {
            return String::new();
        }
        match self.front() {
            Some(Value::String(text)) => text.clone(),
            _ => String::new(),
        }
    }

    fn user_int(&self) -> i64 {
        if self.ty != Type::Number {
            return 0;
        }
        self.front().map(read_int).unwrap_or(0)
    }

    fn user_double(&self) -> f64 {
        if self.ty != Type::Number {
            return 0.0;
        }
        self.front().map(read_double).unwrap_or(0.0)
    }

    fn user_bool(&self) -> bool {
        if self.ty != Type::Boolean {
            return false;
        }
        self.front().map(read_bool).unwrap_or(false)
    }

    fn user_obj(&self) -> Option<Data> {
        if self.ty != Type::Object {
            return None;
        }
        match self.front() {
            Some(Value::Object(obj)) => obj.clone(),
            _ => None,
        }
    }

    fn user_array(&self) -> Option<Array> {
        if self.ty != Type::Array {
            return None;
        }
        match self.front() {
            Some(Value::Array(array)) => array.clone(),
            _ => None,
        }
    }

    fn user_numtype(&self) -> NumType {
        if self.ty != Type::Number {
            return NumType::Invalid;
        }
        match self.front() {
            Some(Value::Int(_)) => NumType::Int,
            Some(Value::Double(_)) => NumType::Double,
            _ => NumType::Invalid,
        }
    }

    fn slot_string(&self, slot: Slot) -> String {
        if self.ty != Type::String {
            return String::new();
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        match value {
            Some(Value::String(text)) => text.clone(),
            _ => String::new(),
        }
    }

    fn slot_int(&self, slot: Slot) -> i64 {
        if self.ty != Type::Number {
            return 0;
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        value.as_ref().map(read_int).unwrap_or(0)
    }

    fn slot_double(&self, slot: Slot) -> f64 {
        if self.ty != Type::Number {
            return 0.0;
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        value.as_ref().map(read_double).unwrap_or(0.0)
    }

    fn slot_bool(&self, slot: Slot) -> bool {
        if self.ty != Type::Boolean {
            return false;
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        value.as_ref().map(read_bool).unwrap_or(false)
    }

    fn slot_obj(&self, slot: Slot) -> Option<Data> {
        if self.ty != Type::Object {
            return None;
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        match value {
            Some(Value::Object(obj)) => obj.clone(),
            _ => None,
        }
    }

    fn slot_array(&self, slot: Slot) -> Option<Array> {
        if self.ty != Type::Array {
            return None;
        }
        let value = match slot {
            Slot::User => &self.user,
            Slot::Default => &self.default,
            Slot::Autoselect => &self.autoselect,
        };
        match value {
            Some(Value::Array(array)) => array.clone(),
            _ => None,
        }
    }

    fn slot_string_ptr(&self, slot: Slot) -> *const std::ffi::c_char {
        if self.ty != Type::String {
            return c"".as_ptr();
        }
        // User getters fall through. Default and autoselect do not.
        if matches!(slot, Slot::User) {
            return self.front_c();
        }
        let cached = match slot {
            Slot::User => &self.user_c,
            Slot::Default => &self.default_c,
            Slot::Autoselect => &self.auto_c,
        };
        let present = match slot {
            Slot::User => self.user.is_some(),
            Slot::Default => self.default.is_some(),
            Slot::Autoselect => self.autoselect.is_some(),
        };
        if present { c_ptr(cached) } else { c"".as_ptr() }
    }
}

/// One setting. The pointer stays stable for the life of the item.
#[derive(Clone)]
pub struct Item {
    pub(crate) inner: Arc<ItemNode>,
}

impl Item {
    fn from_rc(inner: Arc<ItemNode>) -> Self {
        Self { inner }
    }

    pub fn name(&self) -> String {
        self.inner.name.string()
    }

    pub fn ty(&self) -> Type {
        self.inner.borrow().ty
    }

    pub fn numtype(&self) -> NumType {
        self.inner.borrow().user_numtype()
    }

    pub fn has_user_value(&self) -> bool {
        self.inner.borrow().user.is_some()
    }

    pub fn has_default_value(&self) -> bool {
        self.inner.borrow().default.is_some()
    }

    pub fn has_autoselect_value(&self) -> bool {
        self.inner.borrow().autoselect.is_some()
    }

    pub fn get_string(&self) -> String {
        self.inner.borrow().user_string()
    }

    pub fn get_int(&self) -> i64 {
        self.inner.borrow().user_int()
    }

    pub fn get_double(&self) -> f64 {
        self.inner.borrow().user_double()
    }

    pub fn get_bool(&self) -> bool {
        self.inner.borrow().user_bool()
    }

    pub fn get_obj(&self) -> Option<Data> {
        self.inner.borrow().user_obj()
    }

    pub fn get_array(&self) -> Option<Array> {
        self.inner.borrow().user_array()
    }

    pub fn get_autoselect_int(&self) -> i64 {
        self.inner.borrow().slot_int(Slot::Autoselect)
    }

    pub fn get_default_string(&self) -> String {
        self.inner.borrow().slot_string(Slot::Default)
    }

    pub fn get_default_int(&self) -> i64 {
        self.inner.borrow().slot_int(Slot::Default)
    }

    pub fn get_default_double(&self) -> f64 {
        self.inner.borrow().slot_double(Slot::Default)
    }

    pub fn get_default_bool(&self) -> bool {
        self.inner.borrow().slot_bool(Slot::Default)
    }

    pub fn get_default_obj(&self) -> Option<Data> {
        self.inner.borrow().slot_obj(Slot::Default)
    }

    pub fn get_default_array(&self) -> Option<Array> {
        self.inner.borrow().slot_array(Slot::Default)
    }

    pub fn get_autoselect_string(&self) -> String {
        self.inner.borrow().slot_string(Slot::Autoselect)
    }

    pub fn get_autoselect_double(&self) -> f64 {
        self.inner.borrow().slot_double(Slot::Autoselect)
    }

    pub fn get_autoselect_bool(&self) -> bool {
        self.inner.borrow().slot_bool(Slot::Autoselect)
    }

    pub fn get_autoselect_obj(&self) -> Option<Data> {
        self.inner.borrow().slot_obj(Slot::Autoselect)
    }

    pub fn get_autoselect_array(&self) -> Option<Array> {
        self.inner.borrow().slot_array(Slot::Autoselect)
    }

    pub fn set_string(&self, val: Option<&str>) {
        self.write(Slot::User, Value::string(val));
    }

    pub fn set_int(&self, val: i64) {
        self.write(Slot::User, Value::Int(val));
    }

    pub fn set_double(&self, val: f64) {
        self.write(Slot::User, Value::Double(val));
    }

    pub fn set_bool(&self, val: bool) {
        self.write(Slot::User, Value::Bool(val));
    }

    pub fn set_obj(&self, val: Option<&Data>) {
        self.write(Slot::User, Value::Object(val.cloned()));
    }

    pub fn set_array(&self, val: Option<&Array>) {
        self.write(Slot::User, Value::Array(val.cloned()));
    }

    pub fn set_default_string(&self, val: Option<&str>) {
        self.write(Slot::Default, Value::string(val));
    }

    pub fn set_default_int(&self, val: i64) {
        self.write(Slot::Default, Value::Int(val));
    }

    pub fn set_default_double(&self, val: f64) {
        self.write(Slot::Default, Value::Double(val));
    }

    pub fn set_default_bool(&self, val: bool) {
        self.write(Slot::Default, Value::Bool(val));
    }

    pub fn set_default_obj(&self, val: Option<&Data>) {
        self.write(Slot::Default, Value::Object(val.cloned()));
    }

    pub fn set_default_array(&self, val: Option<&Array>) {
        self.write(Slot::Default, Value::Array(val.cloned()));
    }

    pub fn set_autoselect_string(&self, val: Option<&str>) {
        self.write(Slot::Autoselect, Value::string(val));
    }

    pub fn set_autoselect_int(&self, val: i64) {
        self.write(Slot::Autoselect, Value::Int(val));
    }

    pub fn set_autoselect_double(&self, val: f64) {
        self.write(Slot::Autoselect, Value::Double(val));
    }

    pub fn set_autoselect_bool(&self, val: bool) {
        self.write(Slot::Autoselect, Value::Bool(val));
    }

    pub fn set_autoselect_obj(&self, val: Option<&Data>) {
        self.write(Slot::Autoselect, Value::Object(val.cloned()));
    }

    pub fn set_autoselect_array(&self, val: Option<&Array>) {
        self.write(Slot::Autoselect, Value::Array(val.cloned()));
    }

    pub fn unset_user_value(&self) {
        let mut item = self.inner.borrow_mut();
        item.user = None;
        item.user_c = None;
    }

    pub fn unset_default_value(&self) {
        let mut item = self.inner.borrow_mut();
        item.default = None;
        item.default_c = None;
    }

    pub fn unset_autoselect_value(&self) {
        let mut item = self.inner.borrow_mut();
        item.autoselect = None;
        item.auto_c = None;
    }

    fn write(&self, slot: Slot, value: Value) {
        self.inner.borrow_mut().write(slot, value);
    }

    /// Releases this handle and returns the next item, if there is one.
    pub fn next(self) -> Option<Item> {
        let next = self.peek_next();
        drop(self);
        next.map(Item::from_rc)
    }

    fn peek_next(&self) -> Option<Arc<ItemNode>> {
        let parent = self.inner.borrow().parent.clone();
        let parent = parent.and_then(|weak| weak.upgrade())?;
        let data = parent.borrow();
        let idx = data
            .items
            .iter()
            .position(|item| Arc::ptr_eq(item, &self.inner))?;
        data.items.get(idx + 1).cloned()
    }

    /// Drops the object's reference. This handle stays usable.
    pub fn remove(&self) {
        let parent = self.inner.borrow().parent.clone();
        let Some(parent) = parent.and_then(|weak| weak.upgrade()) else {
            return;
        };
        parent.borrow_mut().remove(&self.inner);
        self.inner.borrow_mut().parent = None;
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.inner
            .borrow()
            .parent
            .as_ref()
            .is_some_and(|weak| weak.upgrade().is_some())
    }

    pub(crate) fn name_ptr(&self) -> *const std::ffi::c_char {
        self.inner.name.as_c_str().as_ptr()
    }

    pub(crate) fn string_ptr(&self) -> *const std::ffi::c_char {
        self.inner.borrow().slot_string_ptr(Slot::User)
    }

    pub(crate) fn default_string_ptr(&self) -> *const std::ffi::c_char {
        self.inner.borrow().slot_string_ptr(Slot::Default)
    }

    pub(crate) fn autoselect_string_ptr(&self) -> *const std::ffi::c_char {
        self.inner.borrow().slot_string_ptr(Slot::Autoselect)
    }
}

/// The name index. foldhash is fast on short settings keys and seeded per
/// process, so it keeps some DoS resistance; SipHash, the `HashMap`
/// default, was most of a lookup's cost (#1453).
type NameIndex = HashSet<NameKey, foldhash::fast::RandomState>;

/// An item in the index, looked up by its name bytes.
struct NameKey(Arc<ItemNode>);

impl PartialEq for NameKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.name.bytes() == other.0.name.bytes()
    }
}

impl Eq for NameKey {}

impl Hash for NameKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.name.bytes().hash(state);
    }
}

impl std::borrow::Borrow<[u8]> for NameKey {
    fn borrow(&self) -> &[u8] {
        self.0.name.bytes()
    }
}

/// Up to this many items, a lookup compares the name hashes in order.
/// Settings objects are mostly this small, and for them the index costs
/// more to build than it saves (#1453).
const INDEX_MIN: usize = 16;

pub(crate) struct DataInner {
    /// The items in insertion order, which is the JSON order. Names are
    /// unique: a set of an existing name updates its item.
    items: Vec<Arc<ItemNode>>,
    /// The same items by name once there are more than `INDEX_MIN`, so a
    /// lookup does not walk every item (#1450).
    index: Option<NameIndex>,
    json: Option<CString>,
}

impl DataInner {
    fn find(&self, name: &str) -> Option<&Arc<ItemNode>> {
        let bytes = name.as_bytes();
        match &self.index {
            Some(index) => index.get(bytes).map(|key| &key.0),
            None => {
                let hash = name_hash(bytes);
                self.items.iter().find(|item| item.name.is(hash, bytes))
            }
        }
    }

    fn push(&mut self, item: Arc<ItemNode>) {
        if let Some(index) = &mut self.index {
            index.insert(NameKey(Arc::clone(&item)));
        }
        self.items.push(item);
        if self.index.is_none() && self.items.len() > INDEX_MIN {
            // The key hashes and compares only the item's name, which never
            // changes; the lock inside the item is not part of the key.
            #[allow(clippy::mutable_key_type)]
            let index: NameIndex = self
                .items
                .iter()
                .map(|item| NameKey(Arc::clone(item)))
                .collect();
            self.index = Some(index);
        }
    }

    fn remove(&mut self, item: &Arc<ItemNode>) {
        let Some(at) = self.items.iter().position(|i| Arc::ptr_eq(i, item)) else {
            return;
        };
        let item = self.items.remove(at);
        if let Some(index) = &mut self.index {
            index.remove(item.name.bytes());
        }
    }
}

/// A JSON object of settings. Cloning addrefs; dropping releases.
#[derive(Clone)]
pub struct Data {
    pub(crate) inner: Arc<RwLock<DataInner>>,
}

impl Data {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(DataInner {
                items: Vec::new(),
                index: None,
                json: None,
            })),
        }
    }

    pub fn from_json(text: &str) -> Option<Self> {
        json::load_data(text)
    }

    pub fn from_json_file(path: &Path) -> Option<Self> {
        let text = read_utf8(path)?;
        Self::from_json(&text)
    }

    pub fn from_json_file_safe(path: &Path, backup_ext: &str) -> Option<Self> {
        if let Some(data) = Self::from_json_file(path) {
            return Some(data);
        }
        if backup_ext.is_empty() {
            return None;
        }
        let backup = with_ext(path, backup_ext);
        if !backup.exists() {
            return None;
        }
        let _ = std::fs::remove_file(path);
        std::fs::rename(&backup, path).ok()?;
        Self::from_json_file(path)
    }

    pub fn defaults_from(src: Option<&Data>) -> Data {
        let out = Data::new();
        let Some(src) = src else {
            return out;
        };
        let plan: Vec<(String, Type)> = src
            .inner
            .borrow()
            .items
            .iter()
            .map(|node| {
                let item = node.borrow();
                (node.name.string(), item.ty)
            })
            .collect();
        for (name, ty) in plan {
            match ty {
                Type::Null => {}
                Type::String => out.set_string(&name, Some(&src.get_default_string(&name))),
                Type::Number => match src.item(&name).map(|item| item.numtype()) {
                    Some(NumType::Double) => out.set_double(&name, src.get_default_double(&name)),
                    Some(NumType::Int) => out.set_int(&name, src.get_default_int(&name)),
                    _ => {}
                },
                Type::Boolean => out.set_bool(&name, src.get_default_bool(&name)),
                Type::Object => {
                    let child = src.get_default_obj(&name);
                    let defaults = Data::defaults_from(child.as_ref());
                    out.set_obj(&name, Some(&defaults));
                }
                Type::Array => {
                    let source = src.get_default_array(&name);
                    let defaults = Array::new();
                    if let Some(source) = source {
                        for element in source.elements() {
                            defaults.push(Some(&Data::defaults_from(Some(&element))));
                        }
                    }
                    out.set_array(&name, Some(&defaults));
                }
            }
        }
        out
    }

    pub fn json(&self) -> String {
        self.dump(false, false)
    }

    pub fn json_with_defaults(&self) -> String {
        self.dump(false, true)
    }

    pub fn json_pretty(&self) -> String {
        self.dump(true, false)
    }

    pub fn json_pretty_with_defaults(&self) -> String {
        self.dump(true, true)
    }

    pub fn last_json(&self) -> Option<String> {
        self.inner
            .borrow()
            .json
            .as_ref()
            .map(|text| text.to_string_lossy().into_owned())
    }

    pub fn save_json(&self, path: &Path) -> bool {
        let text = self.json();
        if text.is_empty() {
            return false;
        }
        std::fs::write(path, text.as_bytes()).is_ok()
    }

    pub fn save_json_safe(&self, path: &Path, temp_ext: &str, backup_ext: &str) -> bool {
        self.save_text(path, temp_ext, backup_ext, false)
    }

    pub fn save_json_pretty_safe(&self, path: &Path, temp_ext: &str, backup_ext: &str) -> bool {
        self.save_text(path, temp_ext, backup_ext, true)
    }

    fn save_text(&self, path: &Path, temp_ext: &str, backup_ext: &str, pretty: bool) -> bool {
        if temp_ext.is_empty() {
            return false;
        }
        let text = if pretty {
            self.json_pretty()
        } else {
            self.json()
        };
        if text.is_empty() {
            return false;
        }
        let temp = with_ext(path, temp_ext);
        if std::fs::write(&temp, text.as_bytes()).is_err() {
            return false;
        }
        let backup = if backup_ext.is_empty() {
            None
        } else {
            Some(with_ext(path, backup_ext))
        };
        replace_file(path, &temp, backup.as_deref())
    }

    pub fn set_string(&self, name: &str, val: Option<&str>) {
        self.set_slot(name, Slot::User, Value::string(val));
    }

    pub fn set_int(&self, name: &str, val: i64) {
        self.set_slot(name, Slot::User, Value::Int(val));
    }

    pub fn set_double(&self, name: &str, val: f64) {
        self.set_slot(name, Slot::User, Value::Double(val));
    }

    pub fn set_bool(&self, name: &str, val: bool) {
        self.set_slot(name, Slot::User, Value::Bool(val));
    }

    pub fn set_obj(&self, name: &str, obj: Option<&Data>) {
        self.set_slot(name, Slot::User, Value::Object(obj.cloned()));
    }

    pub fn set_array(&self, name: &str, array: Option<&Array>) {
        self.set_slot(name, Slot::User, Value::Array(array.cloned()));
    }

    pub fn set_default_string(&self, name: &str, val: Option<&str>) {
        self.set_slot(name, Slot::Default, Value::string(val));
    }

    pub fn set_default_int(&self, name: &str, val: i64) {
        self.set_slot(name, Slot::Default, Value::Int(val));
    }

    pub fn set_default_double(&self, name: &str, val: f64) {
        self.set_slot(name, Slot::Default, Value::Double(val));
    }

    pub fn set_default_bool(&self, name: &str, val: bool) {
        self.set_slot(name, Slot::Default, Value::Bool(val));
    }

    pub fn set_default_obj(&self, name: &str, obj: Option<&Data>) {
        self.set_slot(name, Slot::Default, Value::Object(obj.cloned()));
    }

    pub fn set_default_array(&self, name: &str, array: Option<&Array>) {
        self.set_slot(name, Slot::Default, Value::Array(array.cloned()));
    }

    pub fn set_autoselect_string(&self, name: &str, val: Option<&str>) {
        self.set_slot(name, Slot::Autoselect, Value::string(val));
    }

    pub fn set_autoselect_int(&self, name: &str, val: i64) {
        self.set_slot(name, Slot::Autoselect, Value::Int(val));
    }

    pub fn set_autoselect_double(&self, name: &str, val: f64) {
        self.set_slot(name, Slot::Autoselect, Value::Double(val));
    }

    pub fn set_autoselect_bool(&self, name: &str, val: bool) {
        self.set_slot(name, Slot::Autoselect, Value::Bool(val));
    }

    pub fn set_autoselect_obj(&self, name: &str, obj: Option<&Data>) {
        self.set_slot(name, Slot::Autoselect, Value::Object(obj.cloned()));
    }

    pub fn set_autoselect_array(&self, name: &str, array: Option<&Array>) {
        self.set_slot(name, Slot::Autoselect, Value::Array(array.cloned()));
    }

    pub fn get_string(&self, name: &str) -> String {
        self.item(name)
            .map(|item| item.get_string())
            .unwrap_or_default()
    }

    pub fn get_int(&self, name: &str) -> i64 {
        self.item(name).map(|item| item.get_int()).unwrap_or(0)
    }

    pub fn get_double(&self, name: &str) -> f64 {
        self.item(name).map(|item| item.get_double()).unwrap_or(0.0)
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.item(name).map(|item| item.get_bool()).unwrap_or(false)
    }

    pub fn get_obj(&self, name: &str) -> Option<Data> {
        self.item(name).and_then(|item| item.get_obj())
    }

    pub fn get_array(&self, name: &str) -> Option<Array> {
        self.item(name).and_then(|item| item.get_array())
    }

    pub fn get_default_string(&self, name: &str) -> String {
        self.item(name)
            .map(|item| item.get_default_string())
            .unwrap_or_default()
    }

    pub fn get_default_int(&self, name: &str) -> i64 {
        self.item(name)
            .map(|item| item.get_default_int())
            .unwrap_or(0)
    }

    pub fn get_default_double(&self, name: &str) -> f64 {
        self.item(name)
            .map(|item| item.get_default_double())
            .unwrap_or(0.0)
    }

    pub fn get_default_bool(&self, name: &str) -> bool {
        self.item(name)
            .map(|item| item.get_default_bool())
            .unwrap_or(false)
    }

    pub fn get_default_obj(&self, name: &str) -> Option<Data> {
        self.item(name).and_then(|item| item.get_default_obj())
    }

    pub fn get_default_array(&self, name: &str) -> Option<Array> {
        self.item(name).and_then(|item| item.get_default_array())
    }

    pub fn get_autoselect_string(&self, name: &str) -> String {
        self.item(name)
            .map(|item| item.get_autoselect_string())
            .unwrap_or_default()
    }

    pub fn get_autoselect_int(&self, name: &str) -> i64 {
        self.item(name)
            .map(|item| item.get_autoselect_int())
            .unwrap_or(0)
    }

    pub fn get_autoselect_double(&self, name: &str) -> f64 {
        self.item(name)
            .map(|item| item.get_autoselect_double())
            .unwrap_or(0.0)
    }

    pub fn get_autoselect_bool(&self, name: &str) -> bool {
        self.item(name)
            .map(|item| item.get_autoselect_bool())
            .unwrap_or(false)
    }

    pub fn get_autoselect_obj(&self, name: &str) -> Option<Data> {
        self.item(name).and_then(|item| item.get_autoselect_obj())
    }

    pub fn get_autoselect_array(&self, name: &str) -> Option<Array> {
        self.item(name).and_then(|item| item.get_autoselect_array())
    }

    pub fn has_user_value(&self, name: &str) -> bool {
        self.item(name).is_some_and(|item| item.has_user_value())
    }

    pub fn has_default_value(&self, name: &str) -> bool {
        self.item(name).is_some_and(|item| item.has_default_value())
    }

    pub fn has_autoselect_value(&self, name: &str) -> bool {
        self.item(name)
            .is_some_and(|item| item.has_autoselect_value())
    }

    pub fn unset_user_value(&self, name: &str) {
        if let Some(item) = self.item(name) {
            item.unset_user_value();
        }
    }

    pub fn unset_default_value(&self, name: &str) {
        if let Some(item) = self.item(name) {
            item.unset_default_value();
        }
    }

    pub fn unset_autoselect_value(&self, name: &str) {
        if let Some(item) = self.item(name) {
            item.unset_autoselect_value();
        }
    }

    pub fn apply(&self, src: &Data) {
        if Arc::ptr_eq(&self.inner, &src.inner) {
            return;
        }
        let copies: Vec<(String, Value)> = src
            .inner
            .borrow()
            .items
            .iter()
            .filter_map(|node| {
                let item = node.borrow();
                let user = item.user.as_ref()?;
                Some((node.name.string(), copy_user(user)?))
            })
            .collect();
        for (name, value) in copies {
            self.set_slot(&name, Slot::User, value);
        }
    }

    pub fn erase(&self, name: &str) {
        let mut inner = self.inner.borrow_mut();
        let Some(item) = inner.find(name).cloned() else {
            return;
        };
        inner.remove(&item);
        drop(inner);
        item.borrow_mut().parent = None;
    }

    pub fn clear(&self) {
        let items = self.inner.borrow().items.clone();
        for item in items {
            let mut item = item.borrow_mut();
            item.user = None;
            item.user_c = None;
        }
    }

    pub fn first(&self) -> Option<Item> {
        self.inner
            .borrow()
            .items
            .first()
            .map(|item| Item::from_rc(Arc::clone(item)))
    }

    pub fn item(&self, name: &str) -> Option<Item> {
        self.inner
            .borrow()
            .find(name)
            .map(|item| Item::from_rc(Arc::clone(item)))
    }

    pub fn set_vec2(&self, name: &str, val: Vec2) {
        self.set_vec(
            name,
            Slot::User,
            &[("x", f64::from(val.x)), ("y", f64::from(val.y))],
        );
    }

    pub fn set_vec3(&self, name: &str, val: Vec3) {
        self.set_vec(
            name,
            Slot::User,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
            ],
        );
    }

    pub fn set_vec4(&self, name: &str, val: Vec4) {
        self.set_vec(
            name,
            Slot::User,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn set_quat(&self, name: &str, val: Quat) {
        self.set_vec(
            name,
            Slot::User,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn set_default_vec2(&self, name: &str, val: Vec2) {
        self.set_vec(
            name,
            Slot::Default,
            &[("x", f64::from(val.x)), ("y", f64::from(val.y))],
        );
    }

    pub fn set_default_vec3(&self, name: &str, val: Vec3) {
        self.set_vec(
            name,
            Slot::Default,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
            ],
        );
    }

    pub fn set_default_vec4(&self, name: &str, val: Vec4) {
        self.set_vec(
            name,
            Slot::Default,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn set_default_quat(&self, name: &str, val: Quat) {
        self.set_vec(
            name,
            Slot::Default,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn set_autoselect_vec2(&self, name: &str, val: Vec2) {
        self.set_vec(
            name,
            Slot::Autoselect,
            &[("x", f64::from(val.x)), ("y", f64::from(val.y))],
        );
    }

    pub fn set_autoselect_vec3(&self, name: &str, val: Vec3) {
        self.set_vec(
            name,
            Slot::Autoselect,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
            ],
        );
    }

    pub fn set_autoselect_vec4(&self, name: &str, val: Vec4) {
        self.set_vec(
            name,
            Slot::Autoselect,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn set_autoselect_quat(&self, name: &str, val: Quat) {
        self.set_vec(
            name,
            Slot::Autoselect,
            &[
                ("x", f64::from(val.x)),
                ("y", f64::from(val.y)),
                ("z", f64::from(val.z)),
                ("w", f64::from(val.w)),
            ],
        );
    }

    pub fn get_vec2(&self, name: &str) -> Option<Vec2> {
        let obj = self.get_obj(name)?;
        Some(Vec2 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
        })
    }

    pub fn get_vec3(&self, name: &str) -> Option<Vec3> {
        let obj = self.get_obj(name)?;
        Some(Vec3 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
            z: obj.get_double("z") as f32,
            w: 0.0,
        })
    }

    pub fn get_vec4(&self, name: &str) -> Option<Vec4> {
        self.vec4_from(self.get_obj(name)?)
    }

    pub fn get_quat(&self, name: &str) -> Option<Quat> {
        let vec = self.vec4_from(self.get_obj(name)?)?;
        Some(Quat {
            x: vec.x,
            y: vec.y,
            z: vec.z,
            w: vec.w,
        })
    }

    pub fn get_default_vec2(&self, name: &str) -> Option<Vec2> {
        let obj = self.get_default_obj(name)?;
        Some(Vec2 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
        })
    }

    pub fn get_default_vec3(&self, name: &str) -> Option<Vec3> {
        let obj = self.get_default_obj(name)?;
        Some(Vec3 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
            z: obj.get_double("z") as f32,
            w: 0.0,
        })
    }

    pub fn get_default_vec4(&self, name: &str) -> Option<Vec4> {
        self.vec4_from(self.get_default_obj(name)?)
    }

    pub fn get_default_quat(&self, name: &str) -> Option<Quat> {
        let vec = self.vec4_from(self.get_default_obj(name)?)?;
        Some(Quat {
            x: vec.x,
            y: vec.y,
            z: vec.z,
            w: vec.w,
        })
    }

    pub fn get_autoselect_vec2(&self, name: &str) -> Option<Vec2> {
        let obj = self.get_autoselect_obj(name)?;
        Some(Vec2 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
        })
    }

    pub fn get_autoselect_vec3(&self, name: &str) -> Option<Vec3> {
        let obj = self.get_autoselect_obj(name)?;
        Some(Vec3 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
            z: obj.get_double("z") as f32,
            w: 0.0,
        })
    }

    pub fn get_autoselect_vec4(&self, name: &str) -> Option<Vec4> {
        self.vec4_from(self.get_autoselect_obj(name)?)
    }

    pub fn get_autoselect_quat(&self, name: &str) -> Option<Quat> {
        let vec = self.vec4_from(self.get_autoselect_obj(name)?)?;
        Some(Quat {
            x: vec.x,
            y: vec.y,
            z: vec.z,
            w: vec.w,
        })
    }

    pub fn set_fps(&self, name: &str, fps: Fps, option: Option<&str>) {
        self.set_obj(name, Some(&make_fps(fps, option)));
    }

    pub fn set_default_fps(&self, name: &str, fps: Fps, option: Option<&str>) {
        self.set_default_obj(name, Some(&make_fps(fps, option)));
    }

    pub fn set_autoselect_fps(&self, name: &str, fps: Fps, option: Option<&str>) {
        self.set_autoselect_obj(name, Some(&make_fps(fps, option)));
    }

    pub fn get_fps(&self, name: &str, read_option: bool) -> FpsRead {
        match self.get_obj(name) {
            Some(obj) => obj.read_fps(read_option),
            None => FpsRead {
                ok: false,
                fps: None,
                option: None,
            },
        }
    }

    pub fn get_default_fps(&self, name: &str, read_option: bool) -> FpsRead {
        match self.get_default_obj(name) {
            Some(obj) => obj.read_fps(read_option),
            None => FpsRead {
                ok: false,
                fps: None,
                option: None,
            },
        }
    }

    pub fn get_autoselect_fps(&self, name: &str, read_option: bool) -> FpsRead {
        match self.get_autoselect_obj(name) {
            Some(obj) => obj.read_fps(read_option),
            None => FpsRead {
                ok: false,
                fps: None,
                option: None,
            },
        }
    }

    /// Reads a frames object. `read_option` is false when the caller passed a
    /// null option pointer.
    pub fn read_fps(&self, read_option: bool) -> FpsRead {
        if read_option && self.item("option").is_some() {
            return FpsRead {
                ok: true,
                fps: None,
                option: Some(self.get_string("option")),
            };
        }
        if self.item("numerator").is_none() || self.item("denominator").is_none() {
            return FpsRead {
                ok: false,
                fps: None,
                option: None,
            };
        }
        let numerator = clamp_u32(self.get_int("numerator"));
        let denominator = clamp_u32(self.get_int("denominator"));
        let fps = Fps {
            numerator,
            denominator,
        };
        FpsRead {
            ok: numerator != 0 && denominator != 0,
            fps: Some(fps),
            option: None,
        }
    }

    pub(crate) fn option_ptr(&self) -> *const std::ffi::c_char {
        self.item("option")
            .map(|item| item.string_ptr())
            .unwrap_or(std::ptr::null())
    }

    pub(crate) fn json_ptr(&self) -> *const std::ffi::c_char {
        self.inner
            .borrow()
            .json
            .as_ref()
            .map(|text| text.as_ptr())
            .unwrap_or(std::ptr::null())
    }

    fn set_slot(&self, name: &str, slot: Slot, value: Value) {
        // The lookup and the push are one step under the write lock, so two
        // threads that set the same new name make one item (#1451). The
        // item lock comes second, in the usual order.
        let mut inner = self.inner.borrow_mut();
        if let Some(item) = inner.find(name) {
            item.borrow_mut().write(slot, value);
            return;
        }
        // A fresh default still has to record the type. `write` on an empty
        // item does that. Autoselect and user do too.
        let item = ItemInner::create(slot, value, Arc::downgrade(&self.inner));
        inner.push(Arc::new(ItemNode::new(name, item)));
    }

    fn set_vec(&self, name: &str, slot: Slot, pairs: &[(&str, f64)]) {
        let obj = Data::new();
        for (key, value) in pairs {
            obj.set_double(key, *value);
        }
        self.set_slot(name, slot, Value::Object(Some(obj)));
    }

    fn vec4_from(&self, obj: Data) -> Option<Vec4> {
        Some(Vec4 {
            x: obj.get_double("x") as f32,
            y: obj.get_double("y") as f32,
            z: obj.get_double("z") as f32,
            w: obj.get_double("w") as f32,
        })
    }

    fn dump(&self, pretty: bool, with_defaults: bool) -> String {
        self.refresh_json(pretty, with_defaults);
        self.last_json().unwrap_or_default()
    }

    /// Rebuilds the cached JSON text for `pretty` and `with_defaults`.
    ///
    /// The C ABI returns [`Self::json_ptr`] after this, so it must not build a
    /// `String` it would throw away (#1452).
    pub(crate) fn refresh_json(&self, pretty: bool, with_defaults: bool) {
        let text = json::dump_data(self, pretty, with_defaults);
        let mut inner = self.inner.borrow_mut();
        inner.json = text;
    }

    pub(crate) fn rendered_items(&self, with_defaults: bool) -> Vec<(String, Rendered)> {
        self.inner
            .borrow()
            .items
            .iter()
            .filter_map(|node| {
                let item = node.borrow();
                if !with_defaults && item.user.is_none() {
                    return None;
                }
                let rendered = match item.ty {
                    Type::Null => return None,
                    Type::String => Rendered::String(item.user_string()),
                    Type::Number => match item.user_numtype() {
                        NumType::Int => Rendered::Int(item.user_int()),
                        _ => Rendered::Real(item.user_double()),
                    },
                    Type::Boolean => Rendered::Bool(item.user_bool()),
                    Type::Object => Rendered::Object(item.user_obj()),
                    Type::Array => Rendered::Array(item.user_array()),
                };
                Some((node.name.string(), rendered))
            })
            .collect()
    }
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) enum Rendered {
    String(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    Object(Option<Data>),
    Array(Option<Array>),
}

fn copy_user(value: &Value) -> Option<Value> {
    match value {
        Value::Object(None) | Value::Array(None) => None,
        Value::Object(Some(obj)) => {
            let copy = Data::new();
            copy.apply(obj);
            Some(Value::Object(Some(copy)))
        }
        Value::Array(Some(array)) => {
            let copy = Array::new();
            for element in array.elements() {
                let child = Data::new();
                child.apply(&element);
                copy.push(Some(&child));
            }
            Some(Value::Array(Some(copy)))
        }
        other => Some(other.clone()),
    }
}

fn make_fps(fps: Fps, option: Option<&str>) -> Data {
    let obj = Data::new();
    if let Some(option) = option {
        obj.set_string("option", Some(option));
    } else {
        obj.set_double("numerator", f64::from(fps.numerator));
        obj.set_double("denominator", f64::from(fps.denominator));
    }
    obj
}

fn clamp_u32(value: i64) -> u32 {
    if value < 0 {
        0
    } else if value > u32::MAX as i64 {
        u32::MAX
    } else {
        value as u32
    }
}

fn with_ext(path: &Path, ext: &str) -> PathBuf {
    let ext = if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{ext}")
    };
    let mut text = path.as_os_str().to_os_string();
    text.push(ext);
    PathBuf::from(text)
}

fn read_utf8(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let bytes = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        &bytes[..]
    };
    if bytes.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn replace_file(dest: &Path, from: &Path, backup: Option<&Path>) -> bool {
    if let Some(backup) = backup
        && dest.exists()
    {
        let _ = std::fs::remove_file(backup);
        if std::fs::rename(dest, backup).is_err() {
            return false;
        }
    }
    if dest.exists() && std::fs::remove_file(dest).is_err() {
        return false;
    }
    std::fs::rename(from, dest).is_ok()
}

pub(crate) struct ArrayInner {
    items: Vec<Data>,
}

/// A list of objects. Cloning addrefs; dropping releases.
#[derive(Clone)]
pub struct Array {
    pub(crate) inner: Arc<RwLock<ArrayInner>>,
}

impl Array {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ArrayInner { items: Vec::new() })),
        }
    }

    pub fn count(&self) -> usize {
        self.inner.borrow().items.len()
    }

    pub fn count_of(array: Option<&Array>) -> usize {
        array.map(Array::count).unwrap_or(0)
    }

    pub fn item(&self, idx: usize) -> Option<Data> {
        self.inner.borrow().items.get(idx).cloned()
    }

    pub fn push(&self, obj: Option<&Data>) -> usize {
        let Some(obj) = obj else {
            return 0;
        };
        let mut inner = self.inner.borrow_mut();
        inner.items.push(obj.clone());
        inner.items.len() - 1
    }

    pub fn push_of(array: Option<&Array>, obj: Option<&Data>) -> usize {
        match array {
            Some(array) => array.push(obj),
            None => 0,
        }
    }

    pub fn insert(&self, idx: usize, obj: Option<&Data>) {
        let Some(obj) = obj else {
            return;
        };
        let mut inner = self.inner.borrow_mut();
        if idx > inner.items.len() {
            return;
        }
        inner.items.insert(idx, obj.clone());
    }

    pub fn push_array(&self, other: &Array) {
        let extra = other.elements();
        self.inner.borrow_mut().items.extend(extra);
    }

    pub fn erase(&self, idx: usize) {
        let mut inner = self.inner.borrow_mut();
        if idx < inner.items.len() {
            inner.items.remove(idx);
        }
    }

    pub fn elements(&self) -> Vec<Data> {
        self.inner.borrow().items.clone()
    }

    /// Visits each element. A push from the callback is visited.
    pub fn enum_with(&self, mut callback: impl FnMut(&Data)) {
        let mut idx = 0;
        loop {
            let element = {
                let inner = self.inner.borrow();
                if idx >= inner.items.len() {
                    break;
                }
                inner.items[idx].clone()
            };
            callback(&element);
            idx += 1;
        }
    }
}

impl Default for Array {
    fn default() -> Self {
        Self::new()
    }
}

// libobs uses settings objects on several threads (#1451).
const _: () = {
    const fn shareable<T: Send + Sync>() {}
    shareable::<Data>();
    shareable::<Item>();
    shareable::<Array>();
};
