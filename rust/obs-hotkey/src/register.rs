//! Single-hotkey registration policy; storage and weak ownership stay in libobs.

/// Public registration categories in their C ABI order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    Frontend,
    Source,
    Output,
    Encoder,
    Service,
}

/// Operations on opaque owner handles and the existing registration engine.
pub trait Registry {
    type Handle: Copy;
    fn source_private(&mut self, owner: Self::Handle) -> bool;
    fn lock(&mut self) -> bool;
    fn weak(&mut self, kind: Kind, owner: Self::Handle) -> Self::Handle;
    fn context(&mut self, kind: Kind, owner: Self::Handle) -> Self::Handle;
    fn insert(
        &mut self,
        kind: Kind,
        weak: Option<Self::Handle>,
        context: Option<Self::Handle>,
    ) -> usize;
    fn unlock(&mut self);
}

/// Register after admission, retaining the owner only while the core is locked.
/// `None` is the C invalid hotkey ID; the engine's returned ID is otherwise exact.
pub fn register<R: Registry>(
    registry: &mut R,
    kind: Kind,
    owner: Option<R::Handle>,
) -> Option<usize> {
    if kind != Kind::Frontend {
        let owner = owner?;
        if kind == Kind::Source && registry.source_private(owner) {
            return None;
        }
    }
    if !registry.lock() {
        return None;
    }
    let (weak, context) = if kind == Kind::Frontend {
        (None, None)
    } else {
        let owner = owner.expect("non-frontend admission requires an owner");
        (
            Some(registry.weak(kind, owner)),
            Some(registry.context(kind, owner)),
        )
    };
    let id = registry.insert(kind, weak, context);
    registry.unlock();
    Some(id)
}
