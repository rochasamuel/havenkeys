"""Builds the Android app's bundled fonts (apps/android/app/src/main/res/font).

Run from the repository root:
    uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py

Downloads the three OFL families from google/fonts at a pinned commit,
checks their SHA-256, keeps the Latin glyphs HavenKeys needs, fixes the
axes the app does not use, and writes four TTFs. Source Serif 4 declares
the Reserved Font Name "Source", and a subset is a Modified Version under
the OFL, so the serif files are renamed "HavenKeys Serif" (OFL condition 3).
Nothing here runs at build time; the outputs are committed.
"""

import hashlib
import io
import os
import sys
import urllib.request

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

COMMIT = "08dc85da6bca7ae308a6f1d38d0b137465646071"
BASE = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/ofl"

SOURCES = {
    "hanken": (
        "hankengrotesk/HankenGrotesk%5Bwght%5D.ttf",
        "813b3f8fa0965405669a89b38e51bbefd95eef6b8e20d1cb2d8c10cce062662f",
    ),
    "serif": (
        "sourceserif4/SourceSerif4%5Bopsz%2Cwght%5D.ttf",
        "97b2d4da6e3cb494b5a1e66ae176914d852ccabef49e0c02c0df25f3e39aca0b",
    ),
    "serif_italic": (
        "sourceserif4/SourceSerif4-Italic%5Bopsz%2Cwght%5D.ttf",
        "15fbc7e4679489a501998c3669272637a6646388ef7e4bd77eebb5bf967a1f42",
    ),
    "mono": (
        "jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf",
        "48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda",
    ),
}

# Basic Latin, Latin-1, Latin Extended-A/B (names in European languages),
# general punctuation, the euro sign, arrows, minus, bullet and the dot the
# mask is drawn with. Anything else falls back to the system fonts.
UNICODES = (
    "U+0000-024F,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+2000-206F,U+2074,"
    "U+20AC,U+2122,U+2190-2193,U+2212,U+2215,U+2022,U+25CF,U+FEFF,U+FFFD"
)

# (output name, source, axis limits, rename to HavenKeys Serif)
OUTPUTS = [
    # Weights 400-660: body 400/500, controls 550, labels 560, titles 600-660.
    ("hanken_grotesk.ttf", "hanken", {"wght": (400, 660)}, False),
    # Roman 400-520 at optical size 24: monograms and titles.
    ("havenkeys_serif.ttf", "serif", {"opsz": 24, "wght": (400, 520)}, True),
    # The one italic (the unlock headline): 400 only, same optical size as the roman.
    ("havenkeys_serif_italic.ttf", "serif_italic", {"opsz": 24, "wght": 400}, True),
    # Secrets and codes: 500 only.
    ("jetbrains_mono.ttf", "mono", {"wght": 500}, False),
]

SERIF_NAME = "HavenKeys Serif"


def fetch(path: str, sha256: str) -> bytes:
    with urllib.request.urlopen(f"{BASE}/{path}") as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != sha256:
        sys.exit(f"{path}: SHA-256 {digest}, expected {sha256}")
    return data


def subset_font(font: TTFont) -> None:
    options = subset.Options()
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=subset.parse_unicodes(UNICODES))
    subsetter.subset(font)


def rename(font: TTFont, italic: bool) -> None:
    style = "Italic" if italic else "Regular"
    names = {
        1: SERIF_NAME,
        2: style,
        3: f"{SERIF_NAME} {style};HavenKeys",
        4: f"{SERIF_NAME} {style}",
        6: SERIF_NAME.replace(" ", "") + "-" + style,
        16: SERIF_NAME,
        17: style,
    }
    table = font["name"]
    for name_id in (21, 22, 25):
        table.removeNames(nameID=name_id)
    for name_id, value in names.items():
        table.removeNames(nameID=name_id)
        table.setName(value, name_id, 3, 1, 0x409)
    if "fvar" in font:
        # Named instances carry PostScript names that start with the
        # original family; Android does not use them.
        font["fvar"].instances = []
    if "STAT" in font:
        del font["STAT"]
    # The copyright (0) and trademark (7) notices must stay as they are.
    left = [n.nameID for n in table.names if n.nameID not in (0, 7) and "Source" in n.toUnicode()]
    if left:
        sys.exit(f"name IDs still carrying the reserved name: {left}")


def main() -> None:
    out_dir = os.path.join("apps", "android", "app", "src", "main", "res", "font")
    os.makedirs(out_dir, exist_ok=True)
    raw = {key: fetch(path, sha) for key, (path, sha) in SOURCES.items()}
    for name, key, limits, renamed in OUTPUTS:
        font = TTFont(io.BytesIO(raw[key]))
        subset_font(font)
        font = instancer.instantiateVariableFont(font, limits)
        if renamed:
            rename(font, italic=key.endswith("italic"))
        path = os.path.join(out_dir, name)
        # Keep the source's head.modified, so a rerun gives the same bytes.
        font.recalcTimestamp = False
        font.save(path)
        print(f"{path}: {os.path.getsize(path)} bytes")


if __name__ == "__main__":
    main()
