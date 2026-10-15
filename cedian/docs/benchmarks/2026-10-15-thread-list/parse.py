import re
import sys
from collections import defaultdict

rows = defaultdict(lambda: defaultdict(list))
for path in sys.argv[1:]:
    name = None
    for line in open(path):
        m = re.match(r"GPUI bench report \(all observed iterations\): (.+)", line)
        if m:
            name = m.group(1).strip()
            section = None
            continue
        if name is None:
            continue
        m = re.match(r"\s+foreground instructions per iteration: median ([\d.]+) ([KM])", line)
        if m:
            scale = 1e3 if m.group(2) == "K" else 1e6
            rows[name]["fg_minstr"].append(float(m.group(1)) * scale / 1e6)
        m = re.match(r"\s+allocations per iteration: median ([\d.]+) ([KM]?)", line)
        if m and "foreground" not in line:
            scale = {"K": 1e3, "M": 1e6, "": 1}[m.group(2)]
            rows[name]["allocs"].append(float(m.group(1)) * scale)
        if re.match(r"\s+window draw:", line):
            section = "draw"
        elif re.match(r"\s+window dirty-to-draw:", line):
            section = "d2d"
        m = re.match(r"\s+(p50|p99): ([\d.]+)ms", line)
        if m and section == "draw":
            rows[name]["draw_" + m.group(1)].append(float(m.group(2)))


def median(values):
    values = sorted(values)
    return values[len(values) // 2] if values else float("nan")


print(f"{'benchmark':42} {'fg Minstr':>10} {'draw p50 ms':>12} {'draw p99 ms':>12} {'allocs':>10}  runs")
for name in sorted(rows):
    r = rows[name]
    print(
        f"{name:42} {median(r['fg_minstr']):10.2f} {median(r['draw_p50']):12.3f} "
        f"{median(r['draw_p99']):12.3f} {median(r['allocs']):10.0f}  {len(r['fg_minstr'])} "
        f"p50 range {min(r['draw_p50'], default=0):.3f}-{max(r['draw_p50'], default=0):.3f}"
    )
