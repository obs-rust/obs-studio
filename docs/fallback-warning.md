# Fallback warning

Some features of OBS-Studio-Rust have a safe mode and a less safe fallback
mode. When the safe mode cannot work, the feature uses the fallback, so that
the user does not lose the feature. Examples:

| Feature | Safe mode | Fallback |
|---|---|---|
| Browser source (#1092) | Chromium sandbox | No sandbox |
| Streaming (#2205) | RTMPS | RTMP |

The fallback must never be silent (decision on #1092). This document gives
the rules for every fallback, and the shared code that applies them
(obs-rust/obs-studio#2218).

## Rules

1. Log the fallback at error level, once for each time it occurs. The log
   line also goes to stderr.
2. Warn the user in a dialog. The dialog shows what fell back and why, and a
   countdown. When the countdown ends after 3 seconds, the dialog accepts the
   fallback. The countdown pauses while another modal dialog is open, so the
   dialog never accepts unseen.
3. The dialog has an "Accept anyway" button. It has a Cancel button only when
   the caller can stop instead of using the fallback. Then Cancel is the
   default button.
4. Each fallback shows its dialog once in an OBS session. The log line comes
   every time.
5. Do not show a dialog when the user cannot see it: before the main window
   has loaded, and while another modal dialog is open (for example the
   first-run wizard), the warning waits; with a hidden main window (OBS
   started to the tray), the warning accepts at once.
6. Each dialog says: "This may become an error in a future version of OBS."
   A fallback is a temporary help, not a supported mode.
7. Do not fall back when a security check fails. For example, a certificate
   that does not verify is an error, not a reason to use RTMP.

## Wording

Every fallback uses the same words, so the user knows the case at once:

- **Title:** what runs, and without what. For example "Browser source
  without the sandbox", "Streaming unencrypted (RTMP)".
- **Message:** what failed, what OBS does instead, and what the user can do
  about it, in this order. One or two short sentences each.
- **Terms:** "sandbox" for the Chromium sandbox, "unencrypted" for a
  connection without TLS. Do not use "insecure" or "unsafe mode".
- **Buttons:** "Accept anyway" and "Cancel". The frontend supplies them.
- **Id:** `<plugin>.<fallback>`, for example `browser.sandbox`,
  `rtmp.unencrypted`. It is in the log line.

A new fallback uses this helper, not a dialog of its own. Existing
warnings change to this wording case by case.

## How to use it

A plugin calls `obs_frontend_warn_fallback()` from `obs-frontend-api.h`:

```c
static void sandbox_answer(void *param, bool accepted)
{
	/* accepted: continue in the fallback mode. */
}

obs_frontend_warn_fallback("browser.sandbox", obs_module_text("SandboxFallback.Title"),
			   obs_module_text("SandboxFallback.Message"), false, sandbox_answer, data);
```

- `id` is a stable name. Use `<plugin>.<fallback>`.
- The caller translates `title` and `message`.
- `cb` is called once, on the UI thread, never inside the call. Keep `param`
  valid until then. `cb` can be NULL.
- Without the OBS frontend (another host of the plugin), the call logs and
  calls `cb` with `true` inside the call.

The API reference is in
[reference-frontend-api.rst](sphinx/reference-frontend-api.rst).

## Code

| Part | Location |
|---|---|
| Decisions: show or accept, what each end means, the countdown, the log line | `rust/obs-fallback-warning` (Rust, tested in its crate) |
| C ABI of the decisions | `frontend/utility/obs-fallback-warning.h`, linked through `rust/obs-frontend` |
| Queue: log at once, wait for the main window, one dialog at a time | `frontend/utility/FallbackWarning` |
| Dialog | `frontend/dialogs/OBSFallbackWarning` |
| API | `obs_frontend_warn_fallback()` in `frontend/api` and `OBSStudioAPI` |

Without the frontend's Rust (the C rollback build), the frontend logs and
accepts, and shows no dialog.

Tests: `test_fallback_warning_dialog`, `test_fallback_warning_queue` and
`test_frontend_warn_fallback` in `test/cmocka` (Linux).
