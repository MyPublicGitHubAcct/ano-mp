# Licence texts

Standard texts that `scripts/make-notices.py` uses for a shipped package
that names a licence but carries no copy of it (16 crates and the Tauri npm
plugins in 2026-10). `{copyright}` is filled in from the package's authors.

- `MIT.txt`, `BSD-3-Clause.txt`: the SPDX texts, with `{copyright}` for
  the copyright line.
- `MPL-2.0.txt`: Mozilla's text, as the `cssparser` crate ships it.
- `LGPL-2.1.txt`: FFmpeg's `COPYING.LGPLv2.1` (FFmpeg's source isn't kept
  after its build, so the notices can't read it from there).

A package whose chosen licence has no text here makes the script fail;
add the licence's standard text (from https://spdx.org/licenses/) here.
