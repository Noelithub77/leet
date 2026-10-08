# Fonts

Liberation Sans 2.1.5 (Red Hat, SIL Open Font License 1.1, see `LICENSE`), the app's default UI font. Subset to Latin-1 plus a few punctuation and arrow glyphs with `pyftsubset` and stored as WOFF2. Regenerate from `/usr/share/fonts/liberation/LiberationSans-{Regular,Bold}.ttf` with the same `--unicodes` list noted in `docs/development.md`.

Nunito supplies the rounded titles (**[CHOSEN]** by the user). Its bold 700 weight is instantiated from [Google Fonts' Nunito variable source](https://github.com/google/fonts/blob/main/ofl/nunito/Nunito%5Bwght%5D.ttf), then subset to Latin-1 and the same punctuation with FontTools and stored as WOFF2. The font remains under SIL Open Font License 1.1; see `NUNITO-LICENSE`.
