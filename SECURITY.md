# Security Policy

This is keyboard firmware for a local input device. Most bugs should be
reported as normal GitHub issues.

Use a private report instead of a public issue if the problem could allow:

- unexpected host input injection beyond the configured keymap
- unsafe flashing behavior that could overwrite the wrong device
- disclosure of private pairing or host information
- a practical denial-of-service issue affecting connected hosts

If GitHub private vulnerability reporting is enabled for this repository, use
that flow. If it is not enabled, open a minimal issue asking for a private
contact path and do not include exploit details in the issue body.

## Supported Versions

Only the latest released firmware and the current `main` branch are maintained.
Older release assets are kept for reproducibility, but fixes are applied to the
next release rather than backported.

## Firmware Safety

Flashing custom firmware is at your own risk. Always use firmware built for
LaLaPad Gen2 on Seeed Studio XIAO nRF52840 / XIAO BLE, and flash both halves
with the matching central/peripheral artifacts.
