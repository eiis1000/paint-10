# Deterministic shaping fixtures

These unchanged fonts make script coverage and OpenType shaping tests
independent of the host's installed fonts.

- `../fonts/DejaVuSans.ttf`: DejaVu 2.37, also bundled as the application's
  regular default face. Covers Hebrew, Arabic and combining accents. See
  `../fonts/DejaVu-LICENSE.txt`.
- `NotoSansDevanagari.ttf`: Noto Fonts 2026.05.01 from the same environment.
  Compiled into tests only. Covers pre-base vowel reordering and conjunct
  substitution. See `Noto-OFL.txt`.

Expected glyph names and advances in the shaping regressions were checked with
the independent HarfBuzz `hb-shape` executable. The renderer still uses its
ab_glyph outline rasterizer. New text uses standard point sizes; projects without
the size-mode field retain their original pixel-height geometry.
