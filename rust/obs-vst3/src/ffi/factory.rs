use crate::Error;
use crate::platform::{Module, ModuleUse, WINDOWS_COM_ORDER};
use std::ffi::{CString, c_char};
use std::{mem, path::Path};
use vst3::ComPtr;
use vst3::Steinberg::{
    IPluginFactory, IPluginFactory2, IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait,
    IPluginFactoryTrait, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo,
    PFactoryInfo_::FactoryFlags_::kClassesDiscardable, TUID, kResultOk,
};

const AUDIO_MODULE_CLASS: &str = "Audio Module Class";

/// An audio effect or instrument read from a VST3 module factory.
pub struct NativeClass {
    pub name: String,
    pub id: String,
    pub discardable: bool,
    pub sub_categories: String,
}

/// Read factory metadata using the same loader that instantiates plug-ins.
/// Callers that isolate plug-in initialization run this in a helper process.
pub fn enumerate(path: &Path) -> Result<Vec<NativeClass>, Error> {
    let module = Module::open(path, ModuleUse::Scanner)?;
    let factory = module.factory();
    // SAFETY: PFactoryInfo contains only integers and inline arrays.
    let mut info: PFactoryInfo = unsafe { mem::zeroed() };
    // SAFETY: info is a writable SDK layout and the factory remains owned.
    let discardable = unsafe { factory.getFactoryInfo(&mut info) } == kResultOk
        && info.flags & i32::from_ne_bytes(kClassesDiscardable.to_ne_bytes()) != 0;
    Ok(audio_classes(factory)
        .map(|(name, cid, sub_categories)| NativeClass {
            name,
            id: class_id(cid),
            discardable,
            sub_categories,
        })
        .collect())
}

pub(super) fn class_name(factory: &ComPtr<IPluginFactory>, id: &TUID) -> Option<CString> {
    let (name, _, _) = audio_classes(factory).find(|(_, cid, _)| cid == id)?;
    CString::new(name).ok()
}

fn audio_classes(
    factory: &ComPtr<IPluginFactory>,
) -> impl Iterator<Item = (String, TUID, String)> + '_ {
    let v3 = factory.cast::<IPluginFactory3>();
    let v2 = factory.cast::<IPluginFactory2>();
    // SAFETY: The live factory implements the pinned SDK vtable.
    let count = unsafe { factory.countClasses() };
    (0..count).filter_map(move |index| {
        let (category, name, cid, sub_categories) =
            class_info(factory, v2.as_ref(), v3.as_ref(), index)?;
        (category == AUDIO_MODULE_CLASS).then_some((name, cid, sub_categories))
    })
}

pub(super) fn decode_class_id(class_id: &str) -> Option<TUID> {
    if !class_id.is_ascii() || class_id.len() != 32 {
        return None;
    }
    let mut bytes = [0; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = std::ffi::c_char::from_ne_bytes([u8::from_str_radix(
            &class_id[index * 2..index * 2 + 2],
            16,
        )
        .ok()?]);
    }
    if WINDOWS_COM_ORDER {
        bytes[..4].reverse();
        bytes[4..6].reverse();
        bytes[6..8].reverse();
    }
    Some(bytes)
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
    if WINDOWS_COM_ORDER {
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
