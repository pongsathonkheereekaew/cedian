"""Cuts the static faces cedian bundles out of Google Fonts' variable files.

usage: python3 -I script/fonts/instance.py <src> <out>   (needs fonttools)

<src> holds the files from github.com/google/fonts, ofl/<dir>/<file> as listed in JOBS
(fetched 2026-10-22). Each output gets plain names (family "Inter", style "SemiBold") and
weight bits, so GPUI finds it by family and weight on macOS and Linux alike. Copy <out> to
crates/cedian_window/assets/fonts/ with each family's OFL.txt.
"""

import sys, os
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
src, out = sys.argv[1], sys.argv[2]
JOBS = [
 ("inter/Inter[opsz,wght].ttf", "inter/Inter", {"opsz":14}, [(400,"Regular"),(500,"Medium"),(600,"SemiBold"),(700,"Bold")]),
 ("notosansthai/NotoSansThai[wdth,wght].ttf", "noto-sans-thai/NotoSansThai", {"wdth":100}, [(400,"Regular"),(500,"Medium"),(600,"SemiBold"),(700,"Bold")]),
 ("sourceserif4/SourceSerif4[opsz,wght].ttf", "source-serif-4/SourceSerif4", {"opsz":16}, [(400,"Regular"),(600,"SemiBold"),(700,"Bold")]),
 ("notoserifthai/NotoSerifThai[wdth,wght].ttf", "noto-serif-thai/NotoSerifThai", {"wdth":100}, [(400,"Regular"),(600,"SemiBold"),(700,"Bold")]),
]
for path, stem, fixed, weights in JOBS:
    for w, name in weights:
        f = TTFont(os.path.join(src, path))
        inst = instancer.instantiateVariableFont(f, {**fixed, "wght": w}, updateFontNames=True)
        family = {"inter":"Inter","noto-sans-thai":"Noto Sans Thai","source-serif-4":"Source Serif 4","noto-serif-thai":"Noto Serif Thai"}[stem.split("/")[0]]
        rib = name in ("Regular", "Bold")
        names = {1: family if rib else f"{family} {name}", 2: name if rib else "Regular", 4: f"{family} {name}", 6: f"{family.replace(' ','')}-{name}", 16: family, 17: name}
        tbl = inst["name"]
        for nid in (1,2,4,6,16,17,3,21,22,25):
            tbl.removeNames(nameID=nid)
        for nid, v in names.items():
            tbl.setName(v, nid, 3, 1, 0x409)
        tbl.setName(f"cedian;{family.replace(' ','')}-{name}", 3, 3, 1, 0x409)
        os2, head = inst["OS/2"], inst["head"]
        bold = name == "Bold"
        os2.fsSelection = (os2.fsSelection & ~0b1100001) | (0b100000 if bold else 0b1000000)
        head.macStyle = 1 if bold else 0
        dst = os.path.join(out, f"{stem}-{name}.ttf")
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        inst.save(dst)
        n = inst["name"]
        print(dst, os.path.getsize(dst), n.getDebugName(1), "|", n.getDebugName(2), "|", n.getDebugName(16), n.getDebugName(17), inst["OS/2"].usWeightClass)
