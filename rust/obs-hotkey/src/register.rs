//! Admission and owner selection for the five single-hotkey registration APIs.

/// The registerer kinds in `obs-hotkey.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Frontend,
    Source,
    Output,
    Encoder,
    Service,
}

impl Kind {
    /// The unchanged C enum value.
    pub const fn code(self) -> i32 {
        match self {
            Self::Frontend => 0,
            Self::Source => 1,
            Self::Output => 2,
            Self::Encoder => 3,
            Self::Service => 4,
        }
    }
}

/// The existing registry and owner operations. The engine retains allocation,
/// saved binding enrollment, signals and teardown; this API only selects its inputs.
pub trait Registry<R> {
    type Owner: Copy;
    type Weak;
    type Context;
    fn source_private(&mut self, owner: Self::Owner) -> bool;
    fn lock(&mut self) -> bool;
    fn acquire(&mut self, kind: Kind, owner: Self::Owner) -> (Self::Weak, Self::Context);
    fn register(&mut self, kind: Kind, owner: Option<(Self::Weak, Self::Context)>, request: R) -> usize;
    fn unlock(&mut self);
}

/// Register a frontend key or a non-null object key, rejecting private sources
/// before locking. Acquire the weak owner only after the lock succeeds.
pub fn register<B: Registry<R>, R>(
    registry: &mut B,
    kind: Kind,
    owner: Option<B::Owner>,
    request: R,
) -> usize {
    let _ = (registry, kind, owner, request);
    todo!()
}
