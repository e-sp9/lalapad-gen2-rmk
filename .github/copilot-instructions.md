# GitHub Copilot Instructions

This is an RMK firmware project for LaLaPad Gen2.

Follow the repository guidance in `AGENTS.md`:

- Check latest official RMK documentation before changing RMK configuration, dependencies, or build logic.
- Use `keyboard.toml`, `vial.json`, `src/`, and `docs/PORTING.md` as local source of truth.
- Preserve right-half central / left-half peripheral orientation.
- Keep IQS9151, RGB widget, battery, split, and Vial behavior aligned with the current status in `AGENTS.md` and `docs/PORTING.md`.
- Do not suggest committing generated firmware artifacts or local build helper directories.
