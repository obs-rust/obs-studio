//! Public vertex-data layouts; context and device dispatch remain opaque C seams.
use crate::vertexbuffer_create::{self, Backend, Texture, VertexData};
use obs_util::ffi::bmem::{bmalloc, bmemdup};
use std::ffi::c_void;
use std::ptr::NonNull;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextureArray {
    pub width: usize,
    pub array: *mut c_void,
}

#[repr(C)]
pub struct VertexBufferData {
    pub num: usize,
    pub points: *mut c_void,
    pub normals: *mut c_void,
    pub tangents: *mut c_void,
    pub colors: *mut c_void,
    pub num_tex: usize,
    pub tvarray: *mut TextureArray,
}

unsafe extern "C" {
    fn obs_graphics_vertexbuffer_create_context() -> *mut c_void;
    fn obs_graphics_vertexbuffer_create_dispatch(
        context: *mut c_void,
        data: *mut VertexBufferData,
        flags: u32,
    ) -> *mut c_void;
}

struct Device(NonNull<c_void>);

impl Backend for Device {
    type Data = NonNull<VertexBufferData>;
    type Array = *mut c_void;
    type Buffer = *mut c_void;

    fn count(&self, data: Self::Data) -> usize {
        // SAFETY: the shim checked null; the caller supplies a live public struct.
        unsafe { data.as_ref().num }
    }

    fn read(&self, data: Self::Data) -> VertexData<Self::Array> {
        // SAFETY: read is called only for non-null data with nonzero vertices
        // and DUP_BUFFER. Its arrays satisfy the original C input contract.
        unsafe {
            let data = data.as_ref();
            let textures = if data.tvarray.is_null() || data.num_tex == 0 {
                vec![]
            } else {
                std::slice::from_raw_parts(data.tvarray, data.num_tex)
                    .iter()
                    .map(|tv| Texture {
                        width: tv.width,
                        array: tv.array,
                    })
                    .collect()
            };
            VertexData {
                num: data.num,
                arrays: [data.points, data.normals, data.tangents, data.colors]
                    .map(|array| (!array.is_null()).then_some(array)),
                textures,
            }
        }
    }

    fn duplicate(&mut self, array: Self::Array, bytes: usize) -> Self::Array {
        // SAFETY: the caller guarantees readable storage for the original C
        // size_t product. Libobs allocation is required for backend destruction.
        unsafe { bmemdup(array, bytes) }
    }

    fn build(&mut self, data: VertexData<Self::Array>) -> Self::Data {
        // SAFETY: bmalloc aborts on failure and returns aligned libobs storage.
        // Every field is initialized, and all arrays are backend-owned copies.
        unsafe {
            let out = bmalloc(size_of::<VertexBufferData>()).cast::<VertexBufferData>();
            let [points, normals, tangents, colors] =
                data.arrays.map(|a| a.unwrap_or(std::ptr::null_mut()));
            let num_tex = data.textures.len();
            let tvarray = if num_tex == 0 {
                std::ptr::null_mut()
            } else {
                let array =
                    bmalloc(num_tex.wrapping_mul(size_of::<TextureArray>())).cast::<TextureArray>();
                for (i, tv) in data.textures.into_iter().enumerate() {
                    array.add(i).write(TextureArray {
                        width: tv.width,
                        array: tv.array,
                    });
                }
                array
            };
            out.write(VertexBufferData {
                num: data.num,
                points,
                normals,
                tangents,
                colors,
                num_tex,
                tvarray,
            });
            NonNull::new_unchecked(out)
        }
    }

    fn dispatch(&mut self, data: Option<Self::Data>, flags: u32) -> Self::Buffer {
        // SAFETY: the context was validated before touching data; the backend
        // takes ownership of the provided input even when it returns null.
        unsafe {
            obs_graphics_vertexbuffer_create_dispatch(
                self.0.as_ptr(),
                data.map_or(std::ptr::null_mut(), NonNull::as_ptr),
                flags,
            )
        }
    }
}

/// Create a vertex buffer, transferring data (or its deep copy) to the device.
///
/// # Safety
/// A current context must remain live through dispatch. Non-null data and all
/// populated arrays must satisfy graphics.h's count, width and ownership
/// contract. Allocations freed by the backend must use the libobs allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertexbuffer_create(
    data: *mut VertexBufferData,
    flags: u32,
) -> *mut c_void {
    // SAFETY: the C adapter validates/logs without inspecting the input data.
    let mut device =
        NonNull::new(unsafe { obs_graphics_vertexbuffer_create_context() }).map(Device);
    vertexbuffer_create::create(device.as_mut(), NonNull::new(data), flags)
        .unwrap_or(std::ptr::null_mut())
}
