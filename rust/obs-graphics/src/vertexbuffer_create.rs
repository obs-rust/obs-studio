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
    let _ = (backend, data, flags);
    todo!("gs_vertexbuffer_create ownership policy")
}
