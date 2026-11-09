# AArch64 Linux svc table on the UEFI SBSA boot

## Source

Authorized by the human on 2026-11-06. Stack on pull request #13. Gate 18
keeps write, getpid, and exit. This gate adds the rest of the syscall
surface that the x86_64 Linux dispatcher implements, using AArch64 numbers.

## Goal

The aarch64 UEFI image dispatches every x86_64 Linux syscall that has an
AArch64 number. `make uefi-aarch64-guest` shows each one in the boot log.
The image does not link the x86_64 APIC, paging, or Linux syscall path.

## Non-goals

- Merging to fork `main` or opening a pull request on `gmkurtzer/ginux`.
- Starting gate 9.75 or removing `macos-15-intel`.
- x86 numbers that Linux removed from the AArch64 ABI, and x86 numbers whose
  dispatcher returns ENOSYS.
- A second process. `clone` and `clone3` return `EAGAIN`. `exit_group`
  records status 0 and the single thread leaves through `exit`.

## Completed work

- [x] 285 AArch64 syscall numbers are dispatched from `svc #0`.
- [x] The aarch64 image measures an AArch64 controller ELF and starts it.
  `write`, `brk`, and `mmap` return real user memory. The generated EL0
  probe is no longer the boot process.
- [x] The x86_64 `ginux-boot.efi` target is unchanged.

## Remaining work

- [ ] Human review of the pull request.

## Verification plan

`make uefi-aarch64-guest` exits 0. The log contains the gate 18 SBSA lines,
`ginux: embedded controller runtime measurement accepted`,
`ginux: controller elf aarch64`,
`ginux: controller elf loaded`,
`ginux-sbsa: controller write ok`,
`ginux: svc=dispatch nr=64 write result=`,
`ginux: svc=dispatch nr=214 brk result=` with a positive address,
`ginux: svc=dispatch nr=222 mmap result=` with a positive address,
`ginux-sbsa: controller memory ok`, and
`ginux: sbsa arm services ready`, with no `GINUX_KERNEL_PANIC`. The `brk` and
`mmap` results must not be negative.

## Evidence

Observed on 2026-11-06 under AAVMF on
`qemu-system-aarch64 -machine virt,gic-version=3,iommu=smmuv3,acpi=on`.
The first boot counted 285 distinct AArch64 numbers from a generated probe.
The controller boot on 2026-11-07 measured an AArch64 ELF, returned `brk`
`0x20001000` then `0x20002000`, returned `mmap` `0x20100000`, and printed
`ginux-sbsa: controller memory ok` after storing into both pages. QEMU exited 0.

## Not ported

These x86_64 numbers have no AArch64 syscall number. Where the operation
still exists, the AArch64 table has the replacement (`openat`, `newfstatat`,
`ppoll`, `pselect6`, `pipe2`, `dup3`, `getdents64`, `mkdirat`, `unlinkat`,
and the other `*at` calls):

`open`, `stat`, `lstat`, `poll`, `access`, `pipe`, `select`, `dup2`,
`pause`, `alarm`, `fork`, `vfork`, `getdents`, `rename`, `mkdir`, `rmdir`,
`creat`, `link`, `unlink`, `symlink`, `readlink`, `chmod`, `chown`,
`lchown`, `getpgrp`, `utime`, `mknod`, `uselib`, `ustat`, `sysfs`,
`modify_ldt`, `sysctl`, `arch_prctl`, `create_module`, `get_kernel_syms`,
`query_module`, `getpmsg`, `putpmsg`, `afs`, `tuxcall`, `security`, `time`,
`set_thread_area`, `get_thread_area`, `epoll_create`, `epoll_ctl_old`,
`epoll_wait_old`, `epoll_wait`, `utimes`, `vserver`, `inotify_init`,
`futimesat`, `signalfd`, `eventfd`, `uretprobe`.

The x86 dispatcher declares these and returns ENOSYS, so they are not part
of the implemented surface:

`ptrace`, `syslog`, `vhangup`, `pivot_root`, `adjtimex`, `chroot`, `acct`,
`settimeofday`, `umount2`, `swapon`, `swapoff`, `reboot`, `sethostname`,
`setdomainname`, `iopl`, `ioperm`, `init_module`, `delete_module`,
`quotactl`, `clock_settime`, `kexec_load`, `unshare`, `perf_event_open`,
`clock_adjtime`, `setns`, `finit_module`, `kexec_file_load`, `bpf`,
`open_tree`, `move_mount`, `fsopen`, `fsconfig`, `fsmount`, `fspick`,
`quotactl_fd`.

## Review policy

Checkpoint gate. It stays in `gates-todo/` until a human accepts it.
