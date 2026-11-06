"""LLDB embedded-Python driver; imported by LLDB, not the host interpreter."""

import os
import signal

import lldb


def __lldb_init_module(debugger, _internal_dict):  # noqa: C901
    """Launch once, capture fatal stops, pass signals on, and preserve failure."""
    debugger.SetAsync(True)
    target = debugger.GetSelectedTarget()
    info = target.GetLaunchInfo()
    info.SetWorkingDirectory(os.getcwd())
    info.SetLaunchFlags(info.GetLaunchFlags() & ~lldb.eLaunchFlagDisableASLR)
    listener = lldb.SBListener("packaged-startup")
    info.SetListener(listener)
    error = lldb.SBError()
    process = target.Launch(info, error)
    with open(os.environ["OBS_LLDB_LOG"], "w", buffering=1) as log:
        if error.Fail():
            log.write(f"LLDB launch failed: {error}\n")
            os._exit(1)
        log.write(f"Original launch PID: {process.GetProcessID()}\n")

        # The probe sends SIGINT to its Popen child for normal shutdown.
        def forward(signum, _frame):
            os.kill(process.GetProcessID(), signum)

        signal.signal(signal.SIGINT, forward)
        signal.signal(signal.SIGTERM, forward)
        failed = False
        while True:
            event = lldb.SBEvent()
            if not listener.WaitForEvent(1, event):
                continue
            if not lldb.SBProcess.EventIsProcessEvent(event):
                continue
            # Output notifications are process events too, not state changes.
            log.write(process.GetSTDOUT(65536) or "")
            log.write(process.GetSTDERR(65536) or "")
            if not event.GetType() & lldb.SBProcess.eBroadcastBitStateChanged:
                continue
            state = lldb.SBProcess.GetStateFromEvent(event)
            if state == lldb.eStateExited:
                status = process.GetExitStatus()
                log.write(f"Inferior exit: {status}; {process.GetExitDescription()}\n")
                os._exit(status if status > 0 else int(failed or status != 0))
            if state in (lldb.eStateCrashed, lldb.eStateStopped):
                fatal = state == lldb.eStateCrashed
                should_continue = False
                fatal_signal = signal.SIGTRAP
                signals = process.GetUnixSignals()
                for thread in process:
                    reason = thread.GetStopReason()
                    if reason == lldb.eStopReasonSignal:
                        should_continue = True
                        number = thread.GetStopReasonDataAtIndex(0)
                        fatal |= number != signal.SIGINT
                        fatal_signal = number
                        # Deliver the original signal; never suppress or retry.
                        signals.SetShouldSuppress(number, False)
                        signals.SetShouldStop(number, False)
                    elif reason in (lldb.eStopReasonException, lldb.eStopReasonTrace):
                        fatal = True
                        signals.SetShouldSuppress(signal.SIGTRAP, False)
                        signals.SetShouldStop(signal.SIGTRAP, False)
                if fatal:
                    failed = True
                    for command in (
                        "process status",
                        "thread backtrace all",
                        "image list",
                        "thread list",
                    ):
                        result = lldb.SBCommandReturnObject()
                        debugger.GetCommandInterpreter().HandleCommand(
                            command,
                            lldb.SBExecutionContext(
                                process.GetSelectedThread().GetFrameAtIndex(0)
                            ),
                            result,
                        )
                        log.write(
                            f"(lldb) {command}\n{result.GetOutput() or ''}{result.GetError() or ''}"
                        )
                    for thread in process:
                        process.SetSelectedThread(thread)
                        result = lldb.SBCommandReturnObject()
                        debugger.GetCommandInterpreter().HandleCommand(
                            "register read --all",
                            lldb.SBExecutionContext(thread.GetFrameAtIndex(0)),
                            result,
                        )
                        log.write(
                            f"Thread {thread.GetThreadID()} registers:\n{result.GetOutput() or ''}{result.GetError() or ''}"
                        )
                    log.flush()
                    # Leave the debugger before forwarding the fatal signal so
                    # LLDB cannot consume a Mach exception as a trace stop.
                    # Keep it stopped until the fatal signal is pending.
                    pid = process.GetProcessID()
                    detached = process.Detach(True)
                    log.write(f"Detach: {detached}; forwarding signal {fatal_signal}\n")
                    if detached.Fail():
                        # Fail closed: never resume past a fatal stop merely
                        # because this LLDB target cannot detach stopped.
                        process.Kill()
                    else:
                        try:
                            os.kill(pid, fatal_signal)
                            os.kill(pid, signal.SIGCONT)
                        except ProcessLookupError:
                            pass  # The pending original signal already killed it.
                    os._exit(1)
                if not should_continue:
                    continue
                continued = process.Continue()
                if continued.Fail():
                    log.write(f"Cannot continue original signal: {continued}\n")
                    process.Kill()
                    os._exit(1)
            elif state in (lldb.eStateDetached, lldb.eStateInvalid):
                log.write(f"Unexpected debugger state: {state}\n")
                os._exit(1)
