"""Installed regional choices for the first-run UI."""

import ctypes as C
from datetime import datetime
from pathlib import Path
import re
import subprocess
from zoneinfo import ZoneInfo, available_timezones


XKB_LAYOUTS = ("us", "gb", "de", "fr", "es", "it", "br", "jp", "ru")
INPUT_METHODS = ("pinyin", "mozc", "hangul")


def input_methods():
    return [method for method in INPUT_METHODS if (
        Path("/usr/share/fcitx5/inputmethod") / f"{method}.conf"
    ).is_file()]


def keyboards():
    class Names(C.Structure):
        _fields_ = [(name, C.c_char_p) for name in ("rules", "model", "layout", "variant", "options")]

    lib = C.CDLL("libxkbcommon.so.0")

    def bind(name, result, *args):
        fn = getattr(lib, "xkb_" + name)
        fn.restype = result
        fn.argtypes = args
        return fn

    context_new = bind("context_new", C.c_void_p, C.c_int)
    context_unref = bind("context_unref", None, C.c_void_p)
    keymap_new = bind("keymap_new_from_names", C.c_void_p, C.c_void_p, C.POINTER(Names), C.c_int)
    keymap_unref = bind("keymap_unref", None, C.c_void_p)
    key_by_name = bind("keymap_key_by_name", C.c_uint, C.c_void_p, C.c_char_p)
    syms_by_level = bind("keymap_key_get_syms_by_level", C.c_int, C.c_void_p, C.c_uint,
                         C.c_uint, C.c_uint, C.POINTER(C.POINTER(C.c_uint)))
    keysym_unicode = bind("keysym_to_utf32", C.c_uint, C.c_uint)
    keysym_name = bind("keysym_get_name", C.c_int, C.c_uint, C.c_char_p, C.c_size_t)
    # ISO/ANSI and additional Brazilian/Japanese positions use the installed keymap.
    rows = [
        ["TLDE", *[f"AE{i:02}" for i in range(1, 14)]],
        [*[f"AD{i:02}" for i in range(1, 13)], "BKSL"],
        [f"AC{i:02}" for i in range(1, 12)],
        ["LSGT", *[f"AB{i:02}" for i in range(1, 12)]],
    ]
    context = context_new(0)
    if not context:
        raise OSError("could not initialize XKB")
    try:
        result = {}
        for layout in XKB_LAYOUTS:
            model = b"jp106" if layout == "jp" else b"abnt2" if layout == "br" else b"pc105"
            keymap = keymap_new(context, C.byref(Names(b"evdev", model, layout.encode(), b"", b"")), 0)
            if not keymap:
                continue
            try:
                preview = []
                for row in rows:
                    labels = []
                    for name in row:
                        sym = C.POINTER(C.c_uint)()
                        key = key_by_name(keymap, name.encode())
                        if syms_by_level(keymap, key, 0, 0, C.byref(sym)) != 1:
                            continue
                        codepoint = keysym_unicode(sym[0])
                        if codepoint and chr(codepoint).isprintable():
                            labels.append(chr(codepoint))
                        else:
                            buf = C.create_string_buffer(48)
                            if keysym_name(sym[0], buf, len(buf)) > 0:
                                label = buf.value.decode()
                                if label.startswith("dead_"):
                                    labels.append(label.removeprefix("dead_"))
                                elif label == "Zenkaku_Hankaku":
                                    labels.append("半/全")
                    preview.append(labels)
                result[layout] = preview
            finally:
                keymap_unref(keymap)
        return result
    finally:
        context_unref(context)


def coordinates(value):
    match = re.fullmatch(r"([+-])(\d{4}|\d{6})([+-])(\d{5}|\d{7})", value)
    if not match:
        return None

    def degrees(sign, raw, width):
        number = int(raw[:width]) + int(raw[width:width + 2]) / 60 + int(raw[width + 2:] or "0") / 3600
        return -number if sign == "-" else number

    return degrees(*match.groups()[:2], 2), degrees(*match.groups()[2:], 3)


def regional_choices():
    locales = subprocess.check_output(["/usr/bin/locale", "-a"], text=True).splitlines()
    zones = available_timezones()
    points = {}
    for line in Path("/usr/share/zoneinfo/zone.tab").read_text().splitlines():
        if not line.startswith("#"):
            _, position, name, *_ = line.split("\t")
            point = coordinates(position)
            if point is not None:
                points[name] = point
    local_zone = Path("/etc/localtime").resolve().as_posix().partition("/zoneinfo/")[2]
    return {
        "languages": [item for item in locales if "_" in item and item.endswith(".utf8")],
        "keyboards": keyboards(),
        "inputMethods": input_methods(),
        "zones": [{"id": name, "point": points.get(name)} for name in sorted(points.keys() & zones)]
        + [{"id": name, "point": None} for name in sorted(({"UTC", local_zone} & zones) - points.keys())],
        "localZone": local_zone if local_zone in zones else "UTC",
    }


def zone_clock(zone):
    if not isinstance(zone, str) or zone not in available_timezones():
        return {}
    now = datetime.now(ZoneInfo(zone))
    return {"id": zone, "abbr": now.tzname(), "offsetMinutes": int(now.utcoffset().total_seconds() // 60)}
