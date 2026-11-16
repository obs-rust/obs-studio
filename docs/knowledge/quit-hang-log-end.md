---
type: concept
title: Where a quit hang waits, from the last lines of the log
description: "When OBS does not exit after a quit, the last shutdown line in its log shows which step waits: the frontend, obs_shutdown, a module unload, or the process's exit handlers after 'Number of memory leaks'. For the exit handlers, the exit watchdog's line names the library and the plugin that loaded it; otherwise take the stacks and remove plugins one at a time."
sources: ["#3739", "#3933", "#3935", "#3833", "#3846", "#4065", "#4309", "#4525", "#4549", "#4560"]
status: active
---

# Where a quit hang waits, from the last lines of the log

## The shortcut

A quit hang can be in OBS, in a plugin, or in a library that a plugin loads.
Reports of all three look the same: the window closes and the process stays.
Before you try to reproduce one, read the end of the reporter's log. The quit
path writes these lines in this order:

1. `==== Shutting down ====` (`OBSBasic::closeWindow()`, `SHUTDOWN_SEPARATOR` in
   `frontend/widgets/OBSBasic.cpp.hpp`).
2. `All scene data cleared` (`ClearSceneData`, after the
   `OBS_FRONTEND_EVENT_SCRIPTING_SHUTDOWN` event).
3. The modules' own unload lines (for example `[obs-websocket]
   [obs_module_unload] Finished shutting down.`), from `free_module()` in
   `obs_shutdown()` (`libobs/obs.c.h`).
4. `Freeing OBS context data` (`obs_free_data()`, after every module is
   unloaded).
5. `== Profiler Results ==`.
6. `Number of memory leaks: N` (`frontend/obs-main.cpp.hpp`). After this line
   `main()` only returns.
7. `exit-watchdog: exit did not finish in 10 s: ...` (`rust/obs-exit-watchdog`,
   armed in `frontend/obs-main.cpp.hpp` after line 6). Only when `exit()` does
   not finish in 10 s; the process then ends with exit code 124. Linux and
   macOS.

The last line that is present shows where the quit waits:

| Last line in the log | The quit waits in | Example |
|---|---|---|
| 2, and no module unload line | The frontend's own teardown after the scene data: `OBSBasic::applicationShutdown()` destroys the outputs (`outputHandler.reset()`). | #3933: an output that was stopped while it waited to reconnect never finished its stop, and `obs_output_destroy()` waited on it. Fixed in #3935. |
| 3, before `Freeing OBS context data` | A module's `obs_module_unload()`, in a module after the last one that wrote a line. Most modules write nothing when they unload, so the stacks name the module. | #3833: CEF never finished its start, and obs-browser's unload waited for it. Bounded in #3846. |
| 6, and the process still runs | The process's exit handlers, on a build without the watchdog (Windows, or a build before #4309). A normal quit also ends at line 6, but then the process ends. | #3739, before #4309. |
| 7 | The process's exit handlers: static destructors and `atexit` functions of the loaded libraries. OBS has finished. The line names the library the main thread waits in and who loaded it, for example `... is blocking in libremidi.so.4 (loaded by advanced-scene-switcher.so); ending the process`. | #3739: a plugin's static `std::vector<libremidi::observer>` was destroyed at exit, and its PipeWire observer joined a thread that waited on a lock. |

In rows 6 and 7, OBS's own shutdown is complete. libobs calls each module's
`obs_module_unload()` but does not close its library (`free_module()` in
`libobs/obs-module.c.h` leaves `os_dlclose` commented out), and a library that a
plugin opens itself (for example with `QLibrary`) also stays loaded. So the
static objects of every plugin live until `exit()`, and a static object that
owns a thread can wait forever in its destructor.

## Read the watchdog's name

The name is the first library on the main thread's stack that OBS itself did
not load (`culprit` in `rust/obs-exit-watchdog/src/lib.rs`). The text in
brackets says who brought it in:

- `an OBS module`: the library is the module.
- `loaded by <module>`: a module loaded it while it loaded, also with its own
  `dlopen` (Advanced Scene Switcher's sub-plugins). The owner is the last OBS
  module before the library in the process's image list, which is in load
  order (`owners` in `rust/obs-exit-watchdog/src/images.rs`).
- `loaded during the modules' post-load` or `loaded after startup`: it came
  later, so there is no owner. Remove plugins one at a time (below).

Limits, so you do not trust the name too far:

- A library that a plugin thread loads while another module loads gets that
  other module as owner.
- A library that two modules use belongs to the module that loaded it first.
- The name is where the main thread waits, not always the cause. A destructor
  that joins a thread names the joining library; the stacks show what the
  joined thread waits for.
- There is no name when the stack holds only OBS's own frames (the line then
  ends `is blocking; ending the process`), and none on Windows yet.

## Get the stacks

The stacks of the hung process show the waiting frame directly. On Linux, with
OBS still hanging:

```sh
sudo gdb -p "$(pidof obs-studio-rust-gui)" -batch -ex 'thread apply all bt' > obs-hang-stacks.txt 2>&1
```

- `sudo` is necessary on distributions with `kernel.yama.ptrace_scope=1`, for
  example Arch. Without it, gdb cannot attach.
- Use `pidof`, not `pgrep`. `pgrep` matches the 15-character `comm`
  (`obs-studio-rust`), so `pgrep obs-studio-rust-gui` finds nothing. `pgrep -f`
  can also match the shell that runs the command.
- Since #4309 the watchdog ends a row 7 hang after 10 s, so attach within
  that time. There is no switch to turn the watchdog off.
- For rows 6 and 7, the main thread shows `exit()` → a destructor → `join()` or a
  lock. Find the thread that it waits for.

The function names of a distribution package come from its debug package
(`*-debug` on Arch).

## Remove plugins one at a time

When there are no stacks, remove plugins until the hang stops:

- The Plugin Manager (Tools, Plugin Manager) keeps its list in
  `<config dir>/obs-studio-rust/plugin_manager/modules.json`. An entry with
  `"enabled": false` keeps that module from loading. `module_name` is the
  file name without its extension (`advanced-scene-switcher`, not
  `advanced-scene-switcher.so`).
- Some plugins load sub-plugins of their own, which the Plugin Manager does
  not list. Advanced Scene Switcher loads every `.so` in
  `obs-plugins/advanced-scene-switcher-plugins/`. Move one file out of that
  folder at a time.
- Test with an empty scene collection first. If that hangs too, the sources
  are not the cause.
- When a plugin that "worked before" hangs, compare its package history as
  well as its source. In #3739 the plugin code did not change: its Arch
  package changed the libremidi version it builds against.
