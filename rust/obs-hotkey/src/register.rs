//! Admission and owner selection for the five single-hotkey registration APIs.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

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
    fn register(
        &mut self,
        kind: Kind,
        owner: Option<(Self::Weak, Self::Context)>,
        request: R,
    ) -> usize;
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
    let owner = match kind {
        Kind::Frontend => None,
        _ => {
            let Some(owner) = owner else {
                return usize::MAX;
            };
            if kind == Kind::Source && registry.source_private(owner) {
                return usize::MAX;
            }
            Some(owner)
        }
    };
    if !registry.lock() {
        return usize::MAX;
    }
    let fields = owner.map(|owner| registry.acquire(kind, owner));
    let id = registry.register(kind, fields, request);
    registry.unlock();
    id
}
