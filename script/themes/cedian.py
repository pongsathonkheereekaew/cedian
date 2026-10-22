"""Writes crates/cedian_window/assets/themes/cedian.json, the "cedian Dark" and "cedian Light"
Zed themes, from the Warm & Lime tokens in docs/ARCHITECTURE.md (Interface, Color).

Run from the repository root: python3 script/themes/cedian.py
Change a token here and in the architecture's table together; the window reads the tokens Zed
has no key for (frame, field, user.bubble, the tints) from crates/cedian_window/src/tokens.rs.
"""

import json
import pathlib


def rgba(r, g, b, a):
    """A translucent token as Zed's #RRGGBBAA."""
    return f"#{r:02X}{g:02X}{b:02X}{round(255 * a):02X}"


TOKENS = {
    "dark": {
        "frame": "#151412", "app": "#1C1B19", "sheet": "#252421", "raised": "#2C2B28",
        "field": "#262522", "code": "#1E1D1B",
        "hover": rgba(255, 248, 235, 0.045), "sel": rgba(255, 248, 235, 0.075),
        "line": rgba(255, 248, 235, 0.07), "line.strong": rgba(255, 248, 235, 0.13),
        "text": "#EDEBE5", "text.muted": "#ADA99F", "text.faint": "#99958B",
        "accent": "#C6F432", "accent.text": "#C6F432", "diff.add": "#7BD88F",
        "working": "#6FB0FF",
        "failed": "#FF7A66", "warn": "#E8B931",
        # Syntax hues past the four the spec names, kept low in chroma so lime stays the accent.
        "function": "#C3C2FF", "type": "#86D4C4", "tag": "#FFA48F", "constant": "#E8B931",
        "ansi.magenta": "#CFA6FF", "ansi.cyan": "#76D3DB",
    },
    "light": {
        "frame": "#EAEAE6", "app": "#F4F4F1", "sheet": "#FCFCFB", "raised": "#FFFFFF",
        "field": "#ECECE8", "code": "#F7F7F5",
        "hover": rgba(28, 28, 20, 0.04), "sel": rgba(28, 28, 20, 0.065),
        "line": rgba(28, 28, 20, 0.08), "line.strong": rgba(28, 28, 20, 0.14),
        "text": "#1C1C1A", "text.muted": "#5B5B56", "text.faint": "#6C6C66",
        "accent": "#5E8A00", "accent.text": "#4F7500", "diff.add": "#2F7D3F",
        "working": "#1B63BE", "failed": "#C23A22", "warn": "#8F5D00",
        "function": "#4A4FC4", "type": "#00796B", "tag": "#B23A1E", "constant": "#8F5D00",
        "ansi.magenta": "#8A3FC9", "ansi.cyan": "#007C8A",
    },
}


def alpha(color, percent):
    return f"{color}{round(255 * percent / 100):02X}"


def style(t, dark):
    soft = lambda c: alpha(c, 12)
    line = lambda c: alpha(c, 38)
    s = {
        "border": t["line"],
        "border.variant": t["line"],
        "border.focused": t["accent"],
        "border.selected": t["line.strong"],
        "border.transparent": "#00000000",
        "border.disabled": t["line"],
        "elevated_surface.background": t["raised"],
        "surface.background": t["sheet"],
        "background": t["app"],
        "element.background": t["raised"],
        "element.hover": t["hover"],
        "element.active": t["sel"],
        "element.selected": t["sel"],
        "element.disabled": t["raised"],
        "drop_target.background": alpha(t["working"], 20),
        "ghost_element.background": "#00000000",
        "ghost_element.hover": t["hover"],
        "ghost_element.active": t["sel"],
        "ghost_element.selected": t["sel"],
        "ghost_element.disabled": "#00000000",
        "text": t["text"],
        "text.muted": t["text.muted"],
        "text.placeholder": t["text.faint"],
        "text.disabled": t["text.faint"],
        "text.accent": t["accent.text"],
        "icon": t["text"],
        "icon.muted": t["text.muted"],
        "icon.disabled": t["text.faint"],
        "icon.placeholder": t["text.faint"],
        "icon.accent": t["accent.text"],
        "status_bar.background": t["app"],
        "title_bar.background": t["frame"],
        "title_bar.inactive_background": t["frame"],
        "toolbar.background": t["sheet"],
        "tab_bar.background": t["frame"],
        "tab.inactive_background": t["frame"],
        "tab.active_background": t["app"],
        "search.match_background": alpha(t["working"], 30),
        "search.active_match_background": alpha(t["warn"], 40),
        "panel.background": t["app"],
        "panel.focused_border": None,
        "pane.focused_border": None,
        "scrollbar.thumb.background": alpha(t["text.muted"], 30),
        "scrollbar.thumb.hover_background": alpha(t["text.muted"], 50),
        "scrollbar.thumb.border": "#00000000",
        "scrollbar.track.background": "#00000000",
        "scrollbar.track.border": "#00000000",
        "editor.foreground": t["text"],
        "editor.background": t["sheet"],
        "editor.gutter.background": t["sheet"],
        "editor.subheader.background": t["raised"],
        "editor.active_line.background": t["hover"],
        "editor.highlighted_line.background": t["raised"],
        "editor.line_number": t["text.faint"],
        "editor.active_line_number": t["text"],
        "editor.hover_line_number": t["text.muted"],
        "editor.invisible": t["line.strong"],
        "editor.wrap_guide": alpha(t["text"], 5),
        "editor.active_wrap_guide": alpha(t["text"], 10),
        "editor.document_highlight.read_background": soft(t["working"]),
        "editor.document_highlight.write_background": alpha(t["working"], 20),
        "terminal.background": t["code"],
        "terminal.foreground": t["text"],
        "terminal.bright_foreground": t["text"],
        "terminal.dim_foreground": t["text.muted"],
        "link_text.hover": t["working"],
        "version_control.added": t["diff.add"],
        "version_control.modified": t["warn"],
        "version_control.deleted": t["failed"],
        "version_control.word_added": alpha(t["diff.add"], 30),
        "version_control.word_deleted": alpha(t["failed"], 30),
        "version_control.conflict_marker.ours": soft(t["accent"]),
        "version_control.conflict_marker.theirs": soft(t["working"]),
    }
    ansi = {
        "black": t["app"] if dark else t["text"],
        "red": t["failed"], "green": t["diff.add"], "yellow": t["warn"], "blue": t["working"],
        "magenta": t["ansi.magenta"], "cyan": t["ansi.cyan"],
        "white": t["text"] if dark else t["text.faint"],
    }
    for name, color in ansi.items():
        s[f"terminal.ansi.{name}"] = color
        s[f"terminal.ansi.bright_{name}"] = color
        s[f"terminal.ansi.dim_{name}"] = alpha(color, 70)
    s["terminal.ansi.bright_black"] = t["text.faint"]
    statuses = {
        "conflict": t["warn"], "created": t["diff.add"], "deleted": t["failed"],
        "error": t["failed"], "hidden": t["text.faint"], "hint": t["text.muted"],
        "ignored": t["text.faint"], "info": t["working"], "modified": t["warn"],
        "predictive": t["text.faint"], "renamed": t["working"], "success": t["accent"],
        "unreachable": t["text.muted"], "warning": t["warn"],
    }
    for name, color in statuses.items():
        s[name] = color
        s[f"{name}.background"] = soft(color)
        s[f"{name}.border"] = line(color)
    s["players"] = [
        {"cursor": c, "background": c, "selection": alpha(c, 24)}
        for c in [t["accent"], t["working"], t["warn"], t["failed"], t["type"],
                  t["function"], t["tag"], t["ansi.magenta"]]
    ]
    s["players"][0]["selection"] = alpha(t["working"], 28)
    colors = {
        "attribute": t["tag"], "boolean": t["constant"], "comment": t["text.faint"],
        "comment.doc": t["text.muted"], "constant": t["constant"],
        "constructor": t["type"], "embedded": t["text"], "emphasis": t["working"],
        "emphasis.strong": t["working"], "enum": t["type"], "function": t["function"],
        "hint": t["text.faint"], "keyword": t["working"], "label": t["working"],
        "link_text": t["working"], "link_uri": t["type"], "namespace": t["text"],
        "number": t["constant"], "operator": t["text.muted"], "predictive": t["text.faint"],
        "preproc": t["working"], "primary": t["text"], "property": t["text"],
        "punctuation": t["text.muted"], "punctuation.bracket": t["text.muted"],
        "punctuation.delimiter": t["text.muted"], "punctuation.list_marker": t["accent.text"],
        "punctuation.markup": t["text.muted"], "punctuation.special": t["tag"],
        "selector": t["type"], "selector.pseudo": t["working"], "string": t["accent.text"],
        "string.escape": t["text.muted"], "string.regex": t["constant"],
        "string.special": t["constant"], "string.special.symbol": t["constant"],
        "tag": t["tag"], "text.literal": t["accent.text"], "title": t["text"],
        "type": t["type"], "variable": t["text"], "variable.parameter": t["text"],
        "variable.special": t["tag"], "variant": t["type"],
        "diff.plus": t["diff.add"], "diff.minus": t["failed"],
    }
    s["syntax"] = {
        name: {
            "color": color,
            "font_style": None,
            "font_weight": 700 if name in ("emphasis.strong", "title") else None,
        }
        for name, color in colors.items()
    }
    return s


family = {
    "$schema": "https://zed.dev/schema/themes/v0.2.0.json",
    "name": "cedian",
    "author": "cedian",
    "themes": [
        {"name": "cedian Dark", "appearance": "dark", "style": style(TOKENS["dark"], True)},
        {"name": "cedian Light", "appearance": "light", "style": style(TOKENS["light"], False)},
    ],
}
out = pathlib.Path("crates/cedian_window/assets/themes/cedian.json")
out.write_text(json.dumps(family, indent=2) + "\n")
print(out)
