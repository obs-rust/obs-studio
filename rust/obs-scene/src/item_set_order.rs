//! Safe core of `libobs/obs-scene-item-set-order.c.h`:
//! `obs_sceneitem_set_order()`, which moves a scene item one step up or
//! down, or to the top or the bottom of its parent scene's list.
//!
//! C keeps every read and write of the list (`prev`, `next`, `first_item`
//! and `parent`, `detach_sceneitem()` and `attach_sceneitem()`), the full
//! scene lock (video, then audio), the scene reference and the `reorder`
//! signal (`signal_reorder()`); the core asks for each of them through
//! [`SetOrderScene`], in the C's order. Where the item goes back is
//! [`crate::item_order::anchor`] (#2895), so a movement outside
//! `enum obs_order_movement` leaves the item where it was, unlocks and
//! releases without a signal, as the extracted C does with that anchor.

use crate::item_order::{Anchor, anchor};

/// The C-owned scene list `obs_sceneitem_set_order()` works on.
pub trait SetOrderScene {
    /// An item handle (the C's `obs_sceneitem_t *`).
    type Item: Copy;
    /// A scene handle the core holds a reference to.
    type Scene: Copy;
    /// `obs_scene_get_ref(item->parent)`: `None` for a detached item or a
    /// scene that is being destroyed.
    fn parent_ref(&mut self, item: Self::Item) -> Option<Self::Scene>;
    /// Take the scene's full lock (video, then audio).
    fn lock(&mut self, scene: Self::Scene);
    /// `item->next`: the item above, under the lock.
    fn next(&mut self, item: Self::Item) -> Option<Self::Item>;
    /// `item->prev`: the item below, under the lock.
    fn prev(&mut self, item: Self::Item) -> Option<Self::Item>;
    /// `detach_sceneitem()`: unlink the item and clear its parent.
    fn detach(&mut self, item: Self::Item);
    /// `attach_sceneitem()`: link the item into `scene` after `after`, or at
    /// the bottom for `None`.
    fn attach(&mut self, scene: Self::Scene, item: Self::Item, after: Option<Self::Item>);
    /// Release the full lock.
    fn unlock(&mut self, scene: Self::Scene);
    /// `signal_reorder()`: signal `reorder` on the item's parent.
    fn signal_reorder(&mut self, item: Self::Item);
    /// `obs_scene_release()`.
    fn release(&mut self, scene: Self::Scene);
}

/// `obs_sceneitem_set_order()`. A NULL item (`None`) or one whose parent
/// gives no reference returns before any lock. Under the full lock the
/// item's neighbours pick the [`Anchor`]; for every anchor but
/// [`Anchor::Stay`] the item is detached and attached again after
/// `prev->prev`, `next`, `prev`, the last item or at the bottom; then the
/// lock is released, `reorder` signalled and the reference released.
/// [`Anchor::Stay`] unlocks and releases without a signal.
pub fn set_order<S: SetOrderScene>(scene_list: &mut S, item: Option<S::Item>, movement: i32) {
    let Some(item) = item else {
        return;
    };
    let Some(scene) = scene_list.parent_ref(item) else {
        return;
    };

    scene_list.lock(scene);

    let next = scene_list.next(item);
    let prev = scene_list.prev(item);

    let found = anchor(movement, prev.is_some(), next.is_some());

    if found == Anchor::Stay {
        scene_list.unlock(scene);
        scene_list.release(scene);
        return;
    }

    scene_list.detach(item);

    // Each neighbour read here comes after the detach, as in the C, which
    // leaves `prev->prev` and the links above `next` as they were.
    let after = match found {
        Anchor::PrevPrev => prev.and_then(|prev| scene_list.prev(prev)),
        Anchor::Next => next,
        Anchor::Prev => prev,
        Anchor::Last => match next {
            None => prev,
            Some(mut last) => {
                while let Some(above) = scene_list.next(last) {
                    last = above;
                }
                Some(last)
            }
        },
        Anchor::Bottom | Anchor::Stay => None,
    };
    scene_list.attach(scene, item, after);

    scene_list.unlock(scene);

    scene_list.signal_reorder(item);
    scene_list.release(scene);
}
