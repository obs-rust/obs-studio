//! Vertex-array copy policy and ownership transfer to the graphics backend.

pub const DUP_BUFFER: u32 = 1 << 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Texture<A> {
    pub width: usize,
    pub array: A,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VertexData<A> {
    pub num: usize,
    pub arrays: [Option<A>; 4],
    pub textures: Vec<Texture<A>>,
}

/// Handles belong to the backend. `build` and `duplicate` allocate independently
/// of their inputs; `dispatch` takes ownership even when it returns failure.
pub trait Backend {
    type Data: Copy;
    type Array: Copy;
    type Buffer;

    fn count(&self, data: Self::Data) -> usize;
    fn read(&self, data: Self::Data) -> VertexData<Self::Array>;
    fn duplicate(&mut self, array: Self::Array, bytes: usize) -> Self::Array;
    fn build(&mut self, data: VertexData<Self::Array>) -> Self::Data;
    fn dispatch(&mut self, data: Option<Self::Data>, flags: u32) -> Self::Buffer;
}

/// Missing context does nothing; null data, zero vertices and non-duplicate
/// flags pass the original input through without inspecting its arrays.
pub fn create<B: Backend>(
    backend: Option<&mut B>,
    data: Option<B::Data>,
    flags: u32,
) -> Option<B::Buffer> {
    let backend = backend?;
    let data = data.map(|input| {
        if flags & DUP_BUFFER == 0 || backend.count(input) == 0 {
            return input;
        }
        let mut copy = backend.read(input);
        // vec3 occupies 16 bytes, including its padding; colors occupy four.
        // Match the original C size_t products, including wrapping arithmetic.
        for (array, width) in copy.arrays.iter_mut().zip([16usize, 16, 16, 4]) {
            *array = array.map(|src| backend.duplicate(src, width.wrapping_mul(copy.num)));
        }
        for texture in &mut copy.textures {
            let bytes = texture.width.wrapping_mul(4).wrapping_mul(copy.num);
            texture.array = backend.duplicate(texture.array, bytes);
        }
        backend.build(copy)
    });
    Some(backend.dispatch(data, flags))
}
