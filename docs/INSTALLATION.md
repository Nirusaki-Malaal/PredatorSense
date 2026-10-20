# Installed Hardware Stack · 20 October 2026

Machine: **Acer Predator PHN16-71**, kernel **7.2.11-arch1-1**.

- `predatrix_sense` **0.6.0** built and installed through DKMS 3.4.4, signed by
  `DKMS module signing key`, and loaded successfully. Secure Boot is disabled.
- `predator-sense.service` is active and enabled. Its native IPC reports all
  thirteen controls, the correct model and no errors.
- Release daemon, CLI, GTK UI and tray installed in `/usr/local/bin`; hashes
  match the tested release binaries. The installed GUI reports `GskVulkanRenderer`.
  GPU acceleration is retained. GUI and tray have been started for the user.
- Existing AC/battery thermal preferences, four-zone colors, brightness, timeout,
  charge limiter, USB threshold, boot sound and display preference were migrated.
  Calibration is intentionally not replayed.
- Physical performance-button switching was confirmed by the user after loading
  0.6.0. Live sysfs observations included quiet → balanced → balanced-performance
  → performance.
- The old 0.1.0 DKMS package and installed source, old `/opt/predatrix` software,
  old service units, desktop/autostart entries, old binary state files and old
  module-load entry were removed from active locations.
- New module-load and exclusive Acer-driver configuration files are installed.
  The initramfs was rebuilt successfully with `mkinitcpio -p linux`; the previous
  image is retained in the rollback archive. The EC GPE 0x6E correction remains intact.

Rollback files are retained, outside the active installation, at:

```text
/var/lib/predator-sense/migration-24epgpa1
```

This root-only archive includes the previous working Rust 0.5.0 module, the original
installed module, old sources/units/settings, migrated-status snapshots and retired
application files. A first failed DKMS attempt has a separate migration archive;
that attempt left the old driver running. The successful retry fixed the Makefile's
DKMS wrapper selection by separating kernel rules into `Kbuild`.

A real suspend/resume and reboot test remain outstanding. Other laptop models are
outside this branch's support scope. Native settings and event delivery have been
checked; exhaustive firmware feature testing has not been claimed.
