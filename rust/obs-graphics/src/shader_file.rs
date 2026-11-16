//! File-backed shader creation. File ownership and device calls stay at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Vertex,
    Pixel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Context,
    Path,
    Read,
}

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

pub fn create<B: Backend>(backend: &mut B, kind: Kind) -> B::Shader {
    let failure = if !backend.context() {
        Some(Failure::Context)
    } else if !backend.path_present() {
        Some(Failure::Path)
    } else {
        None
    };
    if let Some(failure) = failure {
        backend.log(kind, failure);
        return B::Shader::default();
    }
    let Some(source) = backend.read() else {
        backend.log(kind, Failure::Read);
        return B::Shader::default();
    };
    let shader = backend.create(kind, &source);
    backend.free(source);
    shader
}
