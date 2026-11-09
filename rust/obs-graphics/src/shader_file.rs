//! File-backed shader creation. File ownership and device calls stay at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Vertex, Pixel }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure { Context, Path, Read }

/// A backend owns the original path/error arguments and the loaded allocation.
pub trait Backend {
    type Source;
    type Shader: Default;
    fn context(&self) -> bool;
    fn path_present(&self) -> bool;
    fn read(&mut self) -> Option<Self::Source>;
    fn create(&mut self, kind: Kind, source: &Self::Source) -> Self::Shader;
    fn free(&mut self, source: Self::Source);
    fn log(&mut self, kind: Kind, failure: Failure);
}

pub fn create<B: Backend>(_backend: &mut B, _kind: Kind) -> B::Shader {
    todo!()
}
