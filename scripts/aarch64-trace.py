#!/usr/bin/env python3
"""Run one of upstream's daemon-tracing test scripts on aarch64 (Linux/root).

    aarch64-trace.py <test script> <its arguments...>

The scripts ptrace Ginux itself and read what they find as x86-64: the
daemon's own system calls through PTRACE_GETREGS, and the registers it stages
for a tracee from the argument of its PTRACE_SETREGS. An aarch64 kernel has
neither request. This runs a script unmodified and gives it the same view:

- PTRACE_GETREGS on the traced process returns an x86-64 register file built
  from the aarch64 one: the call's x86-64 number, its arguments in the
  x86-64 argument registers, and x0 as the result.
- PTRACE_SETREGS writes changed argument registers back.
- The daemon stages a tracee's registers with PTRACE_SETREGSET. It passes the
  address of the x86-64 register file it translated them from as a fifth
  argument, which the kernel ignores. That call is shown as the
  PTRACE_SETREGS it is on x86-64, with that address as its data pointer, so
  a script reads the register file the supervisor staged.
- A call that a signal interrupted and the kernel will restart is shown with
  its number and -ERESTARTSYS, as x86-64 shows it at that stop.
- /proc/<pid>/syscall is read with the x86-64 number of the call. A dup3
  without flags is shown as the dup2 the fixture asked for.

On any other machine the script runs as it is.

GINUX_TRACE_TIME_SCALE multiplies the scripts' own time limits, for an
emulated machine. GINUX_TRACE_LOG=1 prints each call shown to the script.
"""
import ctypes
import os
import pathlib
import platform
import re
import runpy
import signal
import sys

PTRACE_GETREGS, PTRACE_SETREGS = 12, 13
PTRACE_GETREGSET, PTRACE_SETREGSET = 0x4204, 0x4205
NT_PRSTATUS, NT_ARM_SYSTEM_CALL = 1, 0x404
PTRACE_PEEKTEXT = 1
HOST_PTRACE = 117
SVC = 0xd4000001
ERESTARTSYS = 512
X86_DUP2, X86_DUP3 = 33, 292
FIELDS = ("r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
          "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()
ARGUMENTS = ("rdi", "rsi", "rdx", "r10", "r8", "r9")
WORD = (1 << 64) - 1


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


class PtRegs(ctypes.Structure):
    _fields_ = [("regs", ctypes.c_ulonglong * 31), ("sp", ctypes.c_ulonglong),
                ("pc", ctypes.c_ulonglong), ("pstate", ctypes.c_ulonglong)]


def x86_numbers(arguments):
    """aarch64 number -> x86-64 number, from the header the fixtures are built with."""
    for argument in arguments:
        header = pathlib.Path(argument).resolve().parent / "syscall-map-aarch64.h"
        if header.is_file():
            rows = re.findall(r"\[(\d+)\] = (\d+) \+ 1,", header.read_text())
            if len(rows) < 300:
                sys.exit(f"aarch64-trace.py: {header} is not the generated table")
            return {int(native): int(x86) for x86, native in rows}
    sys.exit("aarch64-trace.py: no syscall-map-aarch64.h beside the fixture; "
             "run make build-fixtures-arm64")


def number(value):
    return int(getattr(value, "value", value) or 0)


class Ptrace:
    """libc's ptrace, with the two x86-64 register requests supplied."""

    def __init__(self, numbers):
        self.argtypes = self.restype = None  # the scripts assign these
        self.numbers = numbers
        self.shown = {}
        library = ctypes.CDLL(None, use_errno=True)
        self.call = library.ptrace
        self.call.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]
        self.call.restype = ctypes.c_long

    def __call__(self, request, pid, address=0, data=0):
        request, pid, address, data = (number(value) for value in (request, pid, address, data))
        if request == PTRACE_GETREGS:
            return self.get(pid, data)
        if request == PTRACE_SETREGS:
            return self.set(pid, data)
        return self.call(request, pid, address, data)

    def regset(self, request, pid, kind, value):
        vector = Iovec(ctypes.addressof(value), ctypes.sizeof(value))
        return self.call(request, pid, kind, ctypes.addressof(vector))

    def get(self, pid, data):
        raw = PtRegs()
        native = ctypes.c_int(-1)
        if (self.regset(PTRACE_GETREGSET, pid, NT_PRSTATUS, raw) < 0
                or self.regset(PTRACE_GETREGSET, pid, NT_ARM_SYSTEM_CALL, native) < 0):
            return -1
        view = dict.fromkeys(FIELDS, 0)
        for name, value in zip(ARGUMENTS, raw.regs):
            view[name] = value
        view["rax"] = raw.regs[0]
        view["orig_rax"] = (self.numbers.get(native.value, native.value) & WORD)
        view["rip"], view["rsp"], view["eflags"] = raw.pc, raw.sp, raw.pstate
        if native.value == -1 and self.call(PTRACE_PEEKTEXT, pid, raw.pc, 0) & 0xffffffff == SVC:
            # An interrupted call that will restart. Where x86-64 still shows
            # the call and -ERESTARTSYS at the stop, aarch64 has already put
            # the first argument back and pointed pc at the svc.
            view["orig_rax"] = self.numbers.get(raw.regs[8], raw.regs[8])
            view["rax"] = -ERESTARTSYS & WORD
        staging = (native.value == HOST_PTRACE and raw.regs[0] == PTRACE_SETREGSET
                   and raw.regs[2] == NT_PRSTATUS and raw.regs[4] != 0)
        if staging:
            view["rdi"] = PTRACE_SETREGS
            view["rdx"] = 0
            view["r10"] = raw.regs[4]
        self.shown[pid] = (view, staging)
        if os.environ.get("GINUX_TRACE_LOG"):
            print(f"aarch64-trace: pid={pid} call={view['orig_rax']} "
                  + " ".join(f"{view[name]:#x}" for name in ARGUMENTS)
                  + (" staging" if staging else ""), file=sys.stderr, flush=True)
        ctypes.memmove(data, (ctypes.c_ulonglong * len(FIELDS))(*(view[name] for name in FIELDS)),
                       8 * len(FIELDS))
        return 0

    def set(self, pid, data):
        if pid not in self.shown:
            raise RuntimeError("aarch64-trace.py: PTRACE_SETREGS without a PTRACE_GETREGS")
        view, staging = self.shown[pid]
        wanted = dict(zip(FIELDS, (ctypes.c_ulonglong * len(FIELDS)).from_address(data)))
        changed = [name for name in FIELDS if wanted[name] != view[name]]
        if staging or any(name not in ARGUMENTS for name in changed):
            raise RuntimeError(f"aarch64-trace.py: cannot write back {changed}")
        raw = PtRegs()
        if self.regset(PTRACE_GETREGSET, pid, NT_PRSTATUS, raw) < 0:
            return -1
        for index, name in enumerate(ARGUMENTS):
            raw.regs[index] = wanted[name]
        if self.regset(PTRACE_SETREGSET, pid, NT_PRSTATUS, raw) < 0:
            return -1
        self.shown[pid] = (wanted, staging)
        return 0


class Library:
    """A ctypes library whose ptrace is the one above."""

    def __init__(self, library, ptrace):
        self._library = library
        self.ptrace = ptrace

    def __getattr__(self, name):
        return getattr(self._library, name)


def main():
    if len(sys.argv) < 2:
        sys.exit("usage (as root): aarch64-trace.py <test script> <its arguments...>")
    script = sys.argv[1]
    sys.argv = sys.argv[1:]
    scale = float(os.environ.get("GINUX_TRACE_TIME_SCALE", "1"))
    if scale != 1:
        setitimer = signal.setitimer

        def scaled(which, seconds, interval=0.0):
            return setitimer(which, seconds * scale, interval)

        signal.setitimer = scaled
    if platform.machine() == "aarch64":
        numbers = x86_numbers(sys.argv[1:])
        ptrace = Ptrace(numbers)
        load = ctypes.CDLL

        def library(*arguments, **keywords):
            return Library(load(*arguments, **keywords), ptrace)

        ctypes.CDLL = library
        read_text = pathlib.Path.read_text

        def read(path, *arguments, **keywords):
            text = read_text(path, *arguments, **keywords)
            if not re.fullmatch(r"/proc/\d+/syscall", str(path)):
                return text
            fields = text.split()
            if not fields or not fields[0].isdigit():
                return text
            call = numbers.get(int(fields[0]), int(fields[0]))
            if call == X86_DUP3 and len(fields) > 3 and int(fields[3], 16) == 0:
                call = X86_DUP2
            return " ".join([str(call), *fields[1:]]) + "\n"

        pathlib.Path.read_text = read
    runpy.run_path(script, run_name="__main__")


if __name__ == "__main__":
    main()
