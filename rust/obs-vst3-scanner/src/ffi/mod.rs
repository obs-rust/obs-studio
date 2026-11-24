use crate::cache::Class;
use crate::discovery::{self, AUDIO_MODULE_CLASS};
use std::ffi::{CStr, c_char, c_int};
use std::mem;
use std::path::Path;
use std::process::ExitCode;
use vst3::ComPtr;
use vst3::Steinberg::{
    IPluginFactory, IPluginFactory2, IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait,
    IPluginFactoryTrait, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo,
    PFactoryInfo_::FactoryFlags_::kClassesDiscardable, TUID, kResultOk,
};

pub(crate) type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

/// Consume the factory reference returned by a loaded plug-in binary.
pub(crate) unsafe fn classes(factory: *mut IPluginFactory, path: &Path) -> Vec<Class> {
    // SAFETY: The module loader supplies the factory from GetPluginFactory and
    // keeps its module alive until every owned interface below has been dropped.
    let Some(factory) = (unsafe { ComPtr::from_raw(factory) }) else {
        return Vec::new();
    };
    // SAFETY: PFactoryInfo contains only integer fields and inline arrays.
    let mut info: PFactoryInfo = unsafe { mem::zeroed() };
    // SAFETY: info is a writable SDK layout and the factory remains owned.
    let discardable = unsafe { factory.getFactoryInfo(&mut info) } == kResultOk
        && info.flags & i32::from_ne_bytes(kClassesDiscardable.to_ne_bytes()) != 0;
    let v3 = factory.cast::<IPluginFactory3>();
    let v2 = factory.cast::<IPluginFactory2>();
    // SAFETY: The live factory implements the pinned SDK vtable.
    let count = unsafe { factory.countClasses() };
    let mut classes = Vec::new();
    for index in 0..count {
        let Some((category, name, cid, sub_categories)) =
            class_info(&factory, v2.as_ref(), v3.as_ref(), index)
        else {
            continue;
        };
        if category == AUDIO_MODULE_CLASS {
            classes.push(discovery::class(
                path,
                name,
                class_id(cid),
                discardable,
                sub_categories.split('|'),
            ));
        }
    }
    classes
}

fn class_info(
    factory: &ComPtr<IPluginFactory>,
    v2: Option<&ComPtr<IPluginFactory2>>,
    v3: Option<&ComPtr<IPluginFactory3>>,
    index: i32,
) -> Option<(String, String, TUID, String)> {
    if let Some(factory) = v3 {
        // SAFETY: The class-info structures consist of integers and inline arrays.
        let mut info: PClassInfoW = unsafe { mem::zeroed() };
        // SAFETY: info is writable and the caller bounds index by countClasses.
        if unsafe { factory.getClassInfoUnicode(index, &mut info) } == kResultOk {
            let end = info
                .name
                .iter()
                .position(|letter| *letter == 0)
                .unwrap_or(info.name.len());
            return Some((
                text(&info.category),
                String::from_utf16_lossy(&info.name[..end]),
                info.cid,
                text(&info.subCategories),
            ));
        }
    }
    if let Some(factory) = v2 {
        // SAFETY: The class-info structures consist of integers and inline arrays.
        let mut info: PClassInfo2 = unsafe { mem::zeroed() };
        // SAFETY: info is writable and the caller bounds index by countClasses.
        if unsafe { factory.getClassInfo2(index, &mut info) } == kResultOk {
            return Some((
                text(&info.category),
                text(&info.name),
                info.cid,
                text(&info.subCategories),
            ));
        }
    }
    // SAFETY: The class-info structures consist of integers and inline arrays.
    let mut info: PClassInfo = unsafe { mem::zeroed() };
    // SAFETY: info is writable and the caller bounds index by countClasses.
    (unsafe { factory.getClassInfo(index, &mut info) } == kResultOk).then(|| {
        (
            text(&info.category),
            text(&info.name),
            info.cid,
            String::new(),
        )
    })
}

fn text(chars: &[c_char]) -> String {
    let bytes: Vec<_> = chars
        .iter()
        .take_while(|ch| **ch != 0)
        .map(|ch| ch.to_ne_bytes()[0])
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn class_id(cid: TUID) -> String {
    let mut bytes = cid.map(|value| value.to_ne_bytes()[0]);
    if crate::platform::WINDOWS_COM_ORDER {
        bytes[..4].reverse();
        bytes[4..6].reverse();
        bytes[6..8].reverse();
    }
    let mut formatted = String::with_capacity(32);
    use std::fmt::Write;
    for value in bytes {
        let _ = write!(formatted, "{value:02X}");
    }
    formatted
}

/// CMake keeps its native executable target for code signing and installation.
/// The entry point passes a UTF-8 cache path, or null for a missing argument.
#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_scanner_main(cache_path: *const c_char) -> c_int {
    let cache_path = if cache_path.is_null() {
        None
    } else {
        // SAFETY: The launcher supplies a NUL-terminated, live argv string.
        (unsafe { CStr::from_ptr(cache_path) })
            .to_str()
            .ok()
            .map(Path::new)
    };
    i32::from(crate::run_installed(cache_path) != ExitCode::SUCCESS)
}
