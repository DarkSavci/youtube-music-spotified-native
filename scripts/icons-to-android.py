"""Writes the desktop's Lucide icons as Android vector drawables.

The desktop draws crates/app/assets/icons/*.svg; the Android app draws the
same icons, so the two look alike. A vector drawable has paths only, so a
circle, rect or line becomes the path that draws it. Stroke and fill keep the
SVG's; the colour is white, and Compose tints it.

    python -I scripts/icons-to-android.py
"""

import pathlib
import re
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "crates" / "app" / "assets" / "icons"
TARGET = ROOT / "android" / "app" / "src" / "main" / "res" / "drawable"
SVG = "{http://www.w3.org/2000/svg}"


def num(value, default=0.0):
    return float(value) if value not in (None, "") else default


def fmt(value):
    text = f"{value:.3f}".rstrip("0").rstrip(".")
    return text if text not in ("", "-0") else "0"


def path_of(el):
    tag = el.tag.replace(SVG, "")
    a = el.attrib
    if tag == "path":
        return a["d"]
    if tag == "circle":
        cx, cy, r = num(a.get("cx")), num(a.get("cy")), num(a.get("r"))
        return (
            f"M{fmt(cx - r)},{fmt(cy)}a{fmt(r)},{fmt(r)} 0 1,0 {fmt(2 * r)},0"
            f"a{fmt(r)},{fmt(r)} 0 1,0 {fmt(-2 * r)},0"
        )
    if tag == "rect":
        x, y = num(a.get("x")), num(a.get("y"))
        w, h = num(a.get("width")), num(a.get("height"))
        rx = num(a.get("rx"), num(a.get("ry")))
        ry = num(a.get("ry"), rx)
        if rx == 0:
            return f"M{fmt(x)},{fmt(y)}h{fmt(w)}v{fmt(h)}h{fmt(-w)}z"
        return (
            f"M{fmt(x + rx)},{fmt(y)}h{fmt(w - 2 * rx)}"
            f"a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(rx)},{fmt(ry)}v{fmt(h - 2 * ry)}"
            f"a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(-rx)},{fmt(ry)}h{fmt(-(w - 2 * rx))}"
            f"a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(-rx)},{fmt(-ry)}v{fmt(-(h - 2 * ry))}"
            f"a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(rx)},{fmt(-ry)}z"
        )
    if tag == "line":
        return f"M{a['x1']},{a['y1']}L{a['x2']},{a['y2']}"
    if tag in ("polyline", "polygon"):
        points = re.split(r"[\s,]+", a["points"].strip())
        pairs = [f"{points[i]},{points[i + 1]}" for i in range(0, len(points), 2)]
        return "M" + "L".join(pairs) + ("z" if tag == "polygon" else "")
    return None


def convert(svg_path):
    root = ET.parse(svg_path).getroot()
    fill = root.get("fill", "none")
    stroke = root.get("stroke", "none")
    width = root.get("stroke-width", "2")
    out = [
        '<?xml version="1.0" encoding="utf-8"?>',
        f"<!-- {svg_path.name}, Lucide (ISC), from crates/app/assets/icons -->",
        '<vector xmlns:android="http://schemas.android.com/apk/res/android"',
        '    android:width="24dp"',
        '    android:height="24dp"',
        '    android:viewportWidth="24"',
        '    android:viewportHeight="24">',
    ]
    for el in root.iter():
        data = path_of(el)
        if data is None:
            continue
        attrs = [f'android:pathData="{data}"']
        if fill != "none":
            attrs.append('android:fillColor="#FFFFFFFF"')
        if stroke != "none":
            attrs += [
                'android:strokeColor="#FFFFFFFF"',
                f'android:strokeWidth="{width}"',
                'android:strokeLineCap="round"',
                'android:strokeLineJoin="round"',
            ]
        out.append("    <path\n        " + "\n        ".join(attrs) + " />")
    out.append("</vector>")
    return "\n".join(out) + "\n"


def main():
    TARGET.mkdir(parents=True, exist_ok=True)
    for old in TARGET.glob("ic_*.xml"):
        old.unlink()
    for svg in sorted(SOURCE.glob("*.svg")):
        name = "ic_" + svg.stem.replace("-", "_") + ".xml"
        (TARGET / name).write_text(convert(svg), encoding="utf-8", newline="\n")
    print(f"wrote {len(list(SOURCE.glob('*.svg')))} icons to {TARGET}")


main()
