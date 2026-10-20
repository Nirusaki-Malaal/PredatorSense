# PHN16-71 physical mode button — verified fix

On 20 October 2026 the user confirmed that the physical mode button works with
the live Rust module after restoring Embedded Controller event delivery.

## Root cause

The enabled local service
`/etc/systemd/system/disable-wakeup-sources.service` disabled GPE `0x6E` at boot.
The kernel boot log identifies that GPE as the laptop's EC interrupt.
Consequently both the Rust module and a temporarily loaded Linuwu-Sense reference
received no mode-key notifications. SCI and EC interrupt counters were zero.

Manual switching through the original 0.4.7 daemon worked, including firmware
readback and restoration of the previous profile. The failing path was physical
event delivery.

## Applied correction

- Backed up the original service under
  `/var/tmp/predator-ec-interrupt-backup-dx5cc0fe/`.
- Removed its `echo disable > /sys/firmware/acpi/interrupts/gpe6E` command.
- Retained its XHCI/PEG/RP/PXSX wakeup exclusions.
- Reloaded systemd's unit configuration and enabled the EC GPE for this session.
- Restored and ultimately reloaded the cleaned Rust module; the original daemon
  is active. Linuwu was unloaded after the comparison test.

The service correction persists across boots. The Rust module itself remains a
live test; the installed old DKMS module has not been replaced.

## Observed evidence

Before correction:

```text
gpe6E: 0  STS disabled unmasked
Mode button: no WMI callback in either tested driver
```

After correction:

```text
gpe6E: 14  EN enabled unmasked
WMI payload: 07 05 01 00 00 00 00 00
Function: 0x07; key: 0x05
Firmware profile sequence: 4 -> 5 -> 0 -> 1 -> 4 -> 5
```

These firmware values correspond to balanced-performance, performance (Turbo),
quiet, balanced, balanced-performance, performance. The user confirmed successful
physical-button behavior. Battery-mode cycling and sleep/wake behavior remain
untested. The removed command was part of a wakeup workaround, so suspend/wake
needs a separate regression check.

A driver initialization-order discrepancy was also corrected by registering
method endpoints, configuring Acer firmware mode, then registering the event
endpoint. That correction alone did not restore the button; enabling the host
EC interrupt did. Do not add a kernel workaround that silently overrides the
administrator's global interrupt policy.

Read-only diagnostics:

```sh
cat /sys/firmware/acpi/interrupts/gpe6E
cat /sys/firmware/acpi/platform_profile
journalctl -k --grep 'Mode key changed' --no-pager -n 10
systemctl cat disable-wakeup-sources.service
```

GPE `0x6E` is the observed mapping on this laptop; do not apply it blindly to
other models. Linux documents GPE states and enable/disable behavior in its
[ACPI sysfs ABI](https://raw.githubusercontent.com/torvalds/linux/master/Documentation/ABI/testing/sysfs-firmware-acpi).

This record separates the passing build from observed hardware behavior, as
recommended in [A Small Release Evidence Card for State-Change Bugs](https://dev.to/itsmeasadali/a-small-release-evidence-card-for-state-change-bugs-4dp4).
