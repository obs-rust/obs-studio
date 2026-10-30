//! Safe core of `libobs/obs-nix-portal-shortcut-status.c.h`: the status
//! decisions of the GlobalShortcuts portal client (`obs-nix-portal-shortcuts.c`,
//! Wayland with GIO, Linux and FreeBSD).
//!
//! The portal client (still C) asks this code:
//! - [`action`] and [`more`], while it collects the hotkey bindings: how a
//!   combination's description names its count-th action, after `"OBS:"`
//!   (" ", then ", ", up to [`MAX_DESCRIBED_ACTIONS`]), and the N of
//!   `" and N more"`;
//! - [`has_description`], when it takes combinations for a request
//!   (`"OBS"` for one without a description) and looks up the description
//!   a combination was approved with;
//! - [`keep_approved`] and [`rebind`], after the desktop declined a request:
//!   whether `bind_approved()` runs, and whether it binds the approved part
//!   again, logging declined keys or a declined description;
//! - [`after_bind`], after each bind of `sync_shortcuts()`, failed or not:
//!   whether the current combinations become declined or the declined ones
//!   are cleared, and the partly-declined flag; then, after a failed bind
//!   that kept nothing, [`declined`]: whether Settings shows declined or
//!   failed;
//! - [`bound`] and [`desktop_triggers`], for each shortcut the desktop
//!   returns (`BindShortcuts`, `ShortcutsChanged`): whether it is bound, and
//!   `OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS`;
//! - [`can_configure`], when a pending request for the desktop's shortcut
//!   editor is taken and when it is refused: whether the portal has one.
//!
//! It takes the values the client read and returns what it does next, so
//! the client keeps every D-Bus call, the hotkeys mutex, every allocation,
//! string and log. Each decision is split where the C read a value only
//! under a condition, so the client still reads every value where, and
//! only when, it did: `request_declined` after the partly-declined flag is
//! cleared, `portal_gone` and the stopping flag only for a failure, the
//! session and version only for a pending configure. It keeps no state,
//! takes no lock, allocates nothing and logs nothing.
//!
//! No intentional difference from C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// A shortcut's description names this many of its actions; the rest are
/// counted (`" and 2 more"`).
pub const MAX_DESCRIBED_ACTIONS: usize = 3;

/// How a combination's description names an action. The values are the
/// C's (`enum obs_nix_portal_action`), as for each enum here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// After `"OBS:"` and `" "`.
    First = 0,
    /// After `", "`.
    Next = 1,
    /// Not named: counted in `" and N more"`.
    Unnamed = 2,
}

/// The way the `count`-th action of a combination (counted from 1) is
/// named. Characterized, not endorsed: 0 is [`Action::Next`]; the client
/// counts before it asks, so it never asks for 0.
pub fn action(count: usize) -> Action {
    if count > MAX_DESCRIBED_ACTIONS {
        Action::Unnamed
    } else if count == 1 {
        Action::First
    } else {
        Action::Next
    }
}

/// The N of `" and N more"` for a combination with `count` actions, 0 for
/// none.
pub fn more(count: usize) -> usize {
    count.saturating_sub(MAX_DESCRIBED_ACTIONS)
}

/// Whether position `i` of a description array of `num` entries has one.
pub fn has_description(i: usize, num: usize) -> bool {
    i < num
}

/// What `bind_approved()` does after a declined request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebind {
    /// Nothing approved before is in the request: nothing is bound again.
    None = 0,
    /// The approved part is bound again; new keys were declined.
    KeysDeclined = 1,
    /// The approved part is bound again; no key was new, only a
    /// description (the desktop approved the request's keys before).
    DescriptionDeclined = 2,
}

/// `bind_approved()`'s rule for a part of `num_part` approved combinations
/// and `num_declined` other ones: a part that is not empty is bound again.
pub fn rebind(num_part: usize, num_declined: usize) -> Rebind {
    if num_part == 0 {
        Rebind::None
    } else if num_declined > 0 {
        Rebind::KeysDeclined
    } else {
        Rebind::DescriptionDeclined
    }
}

/// Whether a failed bind tries `bind_approved()`: the desktop declined a
/// request that was not empty.
pub fn keep_approved(request_declined: bool, nothing: bool) -> bool {
    request_declined && !nothing
}

/// How `sync_shortcuts()`'s bind went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindResult {
    /// The request was bound; `partly`: it was only part of the current
    /// combinations (keys declined before were left out).
    Bound { partly: bool },
    /// It was not; `kept`: `bind_approved()` bound the approved part again,
    /// and set the status itself.
    Failed { kept: bool },
}

/// What `sync_shortcuts()` does after its bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// The current combinations become the declined ones.
    pub mark_declined: bool,
    /// The declined combinations are cleared.
    pub clear_declined: bool,
    /// The new partly-declined flag, or `None` to leave it.
    pub partly: Option<bool>,
}

/// The outcome of a bind, on the failure and the success path alike.
pub fn after_bind(result: BindResult) -> Outcome {
    match result {
        BindResult::Bound { partly } => Outcome {
            mark_declined: false,
            clear_declined: !partly,
            partly: Some(partly),
        },
        BindResult::Failed { kept: true } => Outcome {
            mark_declined: false,
            clear_declined: false,
            partly: None,
        },
        BindResult::Failed { kept: false } => Outcome {
            mark_declined: true,
            clear_declined: false,
            partly: Some(false),
        },
    }
}

/// Whether a failed bind that kept nothing shows
/// `OBS_HOTKEYS_BACKGROUND_DECLINED`: the desktop declined the request, or
/// it was empty (`nothing`). Otherwise it shows
/// `OBS_HOTKEYS_BACKGROUND_FAILED`, unless the portal left the bus or the
/// client is stopping (the client reads those two only then).
pub fn declined(request_declined: bool, nothing: bool) -> bool {
    request_declined || nothing
}

/// A change to a flag or to a shortcut's bound state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Keep = 0,
    Set = 1,
    Clear = 2,
}

/// What a shortcut the desktop returned does to its bound state.
/// `desktop_keys`: the desktop picks the keys (it has its own shortcut
/// editor, or reported a trigger for any shortcut); `bind_reply`: the
/// `BindShortcuts` reply, not `ShortcutsChanged`; `triggered`: this shortcut
/// has a trigger. On a desktop that picks the keys a shortcut without a
/// trigger is not bound; on any other every returned shortcut is, and only
/// a bind reply changes it.
pub fn bound(desktop_keys: bool, bind_reply: bool, triggered: bool) -> Change {
    if !bind_reply && !desktop_keys {
        Change::Keep
    } else if !desktop_keys || triggered {
        Change::Set
    } else {
        Change::Clear
    }
}

/// What the shortcuts the desktop returned do to
/// `OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS`: set when the desktop picks
/// the keys, cleared only by a bind reply when it doesn't.
pub fn desktop_triggers(desktop_keys: bool, bind_reply: bool) -> Change {
    if desktop_keys {
        Change::Set
    } else if bind_reply {
        Change::Clear
    } else {
        Change::Keep
    }
}

/// The portal version with `ConfigureShortcuts`.
pub const CONFIGURE_VERSION: u32 = 2;

/// Whether the portal has the desktop's shortcut editor. Without it a
/// pending request for the editor is taken without a session, and refused;
/// with it, a request without a session waits for the bind a probe is
/// about to make.
pub fn can_configure(version: u32) -> bool {
    version >= CONFIGURE_VERSION
}
