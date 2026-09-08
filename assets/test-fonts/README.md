# Deterministic shaping fixtures

These unchanged fonts are compiled into tests only, not the application or WASM
release. They make script coverage and OpenType shaping tests independent of
the host's installed fonts.

- `DejaVuSans.ttf`: DejaVu 2.37, from the pinned Nix development environment.
  Covers Hebrew, Arabic and combining accents. See `DejaVu-LICENSE.txt`.
- `NotoSansDevanagari.ttf`: Noto Fonts 2026.05.01 from the same environment.
  Covers pre-base vowel reordering and conjunct substitution. See `Noto-OFL.txt`.

Expected glyph names and advances in the shaping regressions were checked with
the independent HarfBuzz `hb-shape` executable. The renderer still uses its
existing ab_glyph pixel-height scale, not Pango point sizing.
