# PHN16-71 Hardware Audit · 0.6.1

Scope: targets **Acer Predator PHN16-71 exclusively**, verified
from this machine's DMI. The previous broad port and its historical audit remain
preserved in git history. This document describes the specialized architecture.
Signed 0.6.0 is now installed through DKMS and loaded, with the native daemon
and desktop running. The user confirmed physical mode-button switching works.
See [the installation record](INSTALLATION.md).

## Removed support

- Older AMW0/WMID interfaces, model and vendor tables, blacklist exceptions,
  force-model and legacy override module parameters.
- Legacy rfkill, backlight, LED, tablet, accelerometer, debugfs, EC/i8042 and
  old turbo/overclocking paths. Current input-key suppression remains to prevent
  duplicate media/brightness events already delivered by standard input drivers.
- Kernel filesystem persistence and its binary C state format. Durable settings
  belong to the daemon; the kernel retains bounded in-memory resume/power snapshots.
- Old raw-JSON daemon listener, command translation, administrative command execution,
  compatibility aliases and file logging. The native framed socket is the only IPC.
- Nitro sysfs group and broad source reference copies. The current two groups retain
  all twelve PHN16-71 attributes, plus platform profiles, hwmon and input hotkeys.
  The single remaining module parameter is `cycle_gaming_thermal_profile`.

## Fixed / addressed

1. **Locking and callback lifetime:** use the kernel mutex implementation; remove
   the hand-written global `UnsafeCell`, polling lock and unsafe Send/Sync traits.
   Callbacks use a bounded userspace-retriable EBUSY path so subsystem removal
   cannot wait on a callback blocked by the removal lock. Endpoint device refs are
   balanced; teardown drains notifications and joins the worker before freeing it.
2. **Lost worker wakeup:** a completion retains event credits even when a notification
   arrives just before the worker sleeps. Stop permanently completes the wait before
   joining. Dispatch work is bounded per lock acquisition; there is no radio poller.
3. **Slow initialization:** re-query profile capabilities on three short retries;
   stop sleeping seconds under the global state lock with stale capabilities.
4. **Persistence latency / correctness:** no filesystem writes inside the kernel or
   firmware mutex. One bounded storage worker coalesces writes and uses fsync plus
   atomic rename. Failed background flushes retry without requiring another edit. RGB settings save successful requests instead of unstable effect
   readback. Failed restores remain visible and do not replace desired preferences
   with startup readback. Corrupt preferences are backed up before replacement.
5. **Daemon responsiveness:** cached status avoids repeated RGB firmware reads;
   sixteen bounded nonblocking clients, 8 KiB frame limit, 250 ms connection budget
   and partial-write handling prevent a fragmented client blocking others. Full
   refresh is explicit. Native curve configuration validates both arguments before
   writing. No per-request thread, subprocess or synchronous disk flush.
6. **Fan state:** maximum profile uses the correct two-fan command. Failed manual
   writes do not commit a new software mode. Failed curve writes restore hysteresis
   so subsequent retries do not assume an unapplied speed. Curve temperature failure
   keeps the maximum-fan fallback. Turbo no longer overwrites configured fixed
   percentages, so leaving Turbo can restore them.
7. **GUI drafts and leaks:** stable status no longer resets profile, curve, fan,
   lighting or USB edits every second. Active lighting representation selects
   brightness. IPC and telemetry use bounded background workers. Timer cleanup runs
   on close. Histories are limited to sixty samples; overview sampling stops when
   that page is hidden. Tray runs separately without GTK and reaps launched children.
8. **Installer mismatch:** version 0.6.0, native service and corrected desktop names;
   disable old daemon startup to avoid concurrent firmware writers, retain rollback
   state, reject other hardware, retain native DKMS signing and verify its signer.

The physical button's verified original failure was an external system service
which disabled EC GPE 0x6E. That correction remains intact; see
[POWER-BUTTON.md](POWER-BUTTON.md). A driver cannot recover a firmware notification
which ACPI is configured not to deliver.

## Safety boundary

Core, daemon and desktop crate roots forbid unsafe code. Pure kernel policy,
state, model matching, queue, event dispatch and thermal snapshots also forbid it.
There are **no authored unsafe Send/Sync implementations**.

The kernel still requires unsafe C callback, allocation and FFI operations where
safe Rust kernel wrappers are unavailable. Atomic pointers replace manual thread
traits but do not establish ownership by themselves: the kernel mutex and
registration/unregistration lifecycle provide that guarantee. The wake completion
has one owned allocation, initialized before registration and freed after callback
quiescence and worker join. Generated bindings are checked against the target
kernel compiler: seventeen structures and fifty-one accessed fields.

Unsafe does not itself imply a leak, and safe Rust alone cannot prove absence of
resource leaks or firmware errors. This is an explicit, reviewable boundary, not a
claim that all kernel code can be made unsafe-free by deleting the keyword.

The subsequent [UI memory pass](UI-MEMORY.md) retains GPU acceleration and lazy
panel creation; its native desktop measurements supersede the original Xvfb
figures below for estimating accelerated desktop use.

## Validation and practical limits

- Debug and release workspace builds; Clippy with warnings denied; Rust protocol
  validation/fragmentation tests.
- Five host kernel tests: exact-model gate, concurrent queue, FIFO wraparound and
  thermal snapshot validation. Original C/Rust fan policy matches 982 cases.
- Kernel builds against `7.2.11-arch1-1`, including generated ABI checks and valid
  module metadata. No deprecated DKMS CLEAN or legacy signing hook is used.
- Mock integration tests exercise every control, malformed/slow clients, cached
  RGB status, per-power restoration, physical-profile capture, lighting restart,
  calibration exclusion and corrupt-file backup.
- Release run: 1,000 cached IPC requests had p50 0.037 ms / p95 0.060 ms,
  daemon RSS stayed 2,776 KiB and descriptors stayed five.
  One debug run had p50 0.056 ms / p95 0.158 ms;
  daemon RSS stayed 3,456 KiB, descriptors stayed five. A later expanded test
  measured 3,472 KiB with the same unchanged descriptor count. These are mock-IPC
  measurements, not firmware latency measurements or proof of indefinite stability.
- GTK screenshots inspected for all five pages using Xvfb and mock IPC. Tray
  D-Bus registration, menu, cycle request, singleton and quit pass; debug tray
  RSS measured 8,716 KiB (debug) and 4,628 KiB (release), with no GTK dependency.
  A GTK fan edit survived status polling and reached native IPC in an automated test.
  GTK's own process RSS was much
  larger (~231 MiB in that debug/Xvfb environment); no claim is made that the GTK
  renderer has become small. A release preview measured 233,428 KiB RSS,
  96,422 KiB proportional set size and 67,144 KiB private memory. Keeping the GTK
  process out of the tray avoids retaining it for background tray access.
- Native DKMS build, signing and installation pass on this laptop. The DKMS
  KERNELRELEASE invocation exposed a wrapper bug: kernel rules now live in a
  dedicated Kbuild file, so the external Makefile always generates bindings first.
  Installer preflight, desktop entry and shell syntax pass.
  Systemd unit verification against the build tree checks syntax; the installed
  `/usr/local/bin/predator-daemon` is absent until installation.

Hardware-verified so far: loading the signed module, native daemon reads/restores,
settings migration, GUI/tray startup and physical button switching. Still required:
AC/battery changes, all RGB modes,
manual/curve cooling, runtime WMI removal/rebind, repeated suspend/resume and reboot
persistence. Test real MOK enrollment and DKMS rebuild on additional supported
Rust-enabled kernels. Arbitrary Linux kernels and other laptop models are outside
this branch's support scope. Firmware transactions can still block while the WMI
call runs; safe userspace and nonblocking sockets cannot impose a firmware timeout.
