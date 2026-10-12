"""GUI 探针：起临时实例 → 读窗口尺寸 → 截窗口内容 → 发按键 → 收尾。

只用于本地验证 Slint 界面，Windows only，纯 ctypes，无第三方依赖。
改 `src/cockpit.slint` 之后用它替代「起 exe 用眼睛看」：能拿到真实像素尺寸，
能在被其他窗口遮挡时照样截到窗口自身内容（PrintWindow）。

    python tools/ui_probe.py launch                 # 起临时实例并打印 PID
    python tools/ui_probe.py rect <pid>             # 窗口尺寸 + DPI
    python tools/ui_probe.py max <pid>              # 最大化（验证超宽屏布局）
    python tools/ui_probe.py size <pid> <w> <h>     # 改成指定窗口尺寸（含边框）
    python tools/ui_probe.py shot <pid> [scale] [name] [x,y,w,h]
    python tools/ui_probe.py key <pid> <vk-16进制> [次数]
    python tools/ui_probe.py kill <pid>

`scale` 是整数下采样倍率（默认 2，即 1916px 的窗口截成 958px）。截图落 `.scratch/`。
`x,y,w,h` 是可选裁剪区（下采样后的坐标），用来盯住某个局部放大看。

发按键走 PostMessage，只影响目标窗口，不会打扰前台程序。
`key 52` 是 R 键（分身没运行时按它只写一条日志，适合刷日志量做显示测试）。
"""

import ctypes
import struct
import subprocess
import sys
import time
import zlib
from ctypes import wintypes
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target" / "release" / "antigravity-lander.exe"
if not EXE.exists():
    EXE = ROOT / "target" / "release" / "multi-antigravity-rust.exe"
SHOT_DIR = ROOT / ".scratch"

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

SRCCOPY = 0x00CC0020
DIB_RGB_COLORS = 0
BI_RGB = 0
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101


class RECT(ctypes.Structure):
    _fields_ = [("left", wintypes.LONG), ("top", wintypes.LONG),
                ("right", wintypes.LONG), ("bottom", wintypes.LONG)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD), ("biWidth", wintypes.LONG), ("biHeight", wintypes.LONG),
        ("biPlanes", wintypes.WORD), ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD), ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", wintypes.LONG), ("biYPelsPerMeter", wintypes.LONG),
        ("biClrUsed", wintypes.DWORD), ("biClrImportant", wintypes.DWORD),
    ]


def find_window(pid, min_width=200):
    kernel32 = ctypes.WinDLL("kernel32")
    access = 0x0040 | 0x0001 | 0x0100
    desktops_to_try = []

    cur_desk = user32.GetThreadDesktop(kernel32.GetCurrentThreadId())
    if cur_desk:
        desktops_to_try.append((None, cur_desk))

    h_input = user32.OpenInputDesktop(0, False, access)
    if h_input:
        desktops_to_try.append((None, h_input))

    h_def = user32.OpenDesktopW("default", 0, False, access)
    if h_def:
        desktops_to_try.append((None, h_def))

    CB_DESK = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.LPCWSTR, wintypes.LPARAM)
    CB_WINSTA = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.LPCWSTR, wintypes.LPARAM)

    def cb_winsta(name, _):
        hw = user32.OpenWindowStationW(name, False, 0x037f)
        if hw:
            user32.SetProcessWindowStation(hw)
            def cb_desk(dname, _):
                hd = user32.OpenDesktopW(dname, 0, False, access)
                if hd:
                    desktops_to_try.append((hw, hd))
                return True
            user32.EnumDesktopsW(hw, CB_DESK(cb_desk), 0)
        return True
    user32.EnumWindowStationsW(CB_WINSTA(cb_winsta), 0)

    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def cb(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            rect = RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(rect))
            if rect.right - rect.left > min_width:
                found.append((hwnd, rect))
                return False
        return True

    for hw, d in desktops_to_try:
        if hw:
            user32.SetProcessWindowStation(hw)
        user32.SetThreadDesktop(d)
        user32.EnumDesktopWindows(d, cb, 0)
        if found:
            return found[0]

    user32.EnumWindows(cb, 0)
    return found[0] if found else None


def grab(hwnd, rect, scale, crop=None):
    x, y = rect.left, rect.top
    w, h = rect.right - x, rect.bottom - y
    screen = user32.GetDC(0)
    mem = gdi32.CreateCompatibleDC(screen)
    bitmap = gdi32.CreateCompatibleBitmap(screen, w, h)
    gdi32.SelectObject(mem, bitmap)

    if not user32.PrintWindow(hwnd, mem, 2):
        gdi32.BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY)

    info = BITMAPINFOHEADER()
    info.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    info.biWidth = w
    info.biHeight = -h
    info.biPlanes = 1
    info.biBitCount = 32
    info.biCompression = BI_RGB
    buf = ctypes.create_string_buffer(w * h * 4)
    gdi32.GetDIBits(mem, bitmap, 0, h, buf, ctypes.byref(info), DIB_RGB_COLORS)

    out_w, out_h = w // scale, h // scale
    cx, cy, cw, ch = crop if crop else (0, 0, out_w, out_h)
    cx = max(0, min(cx, out_w - 1))
    cy = max(0, min(cy, out_h - 1))
    cw = max(1, min(cw, out_w - cx))
    ch = max(1, min(ch, out_h - cy))
    rows = bytearray()
    for oy in range(cy, cy + ch):
        rows.append(0)
        base = oy * scale * w * 4
        for ox in range(cx, cx + cw):
            sx = ox * scale
            b, g, r, _a = buf[base + sx * 4: base + sx * 4 + 4]
            rows += bytes((r, g, b))
    out_w, out_h = cw, ch

    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(mem)
    user32.ReleaseDC(0, screen)
    return out_w, out_h, bytes(rows), w, h


def png_chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def write_png(path, w, h, pixels):
    data = b"\x89PNG\r\n\x1a\n"
    data += png_chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    data += png_chunk(b"IDAT", zlib.compress(pixels, 6))
    data += png_chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return len(data)


def cmd_launch():
    if not EXE.exists():
        print("先 cargo build --release")
        return 1
    proc = subprocess.Popen([str(EXE)], cwd=str(ROOT), creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP)
    print(proc.pid)
    return 0


def cmd_rect(pid):
    user32.SetProcessDPIAware()
    got = find_window(pid)
    if not got:
        print("window not found")
        return 1
    hwnd, rect = got
    w, h = rect.right - rect.left, rect.bottom - rect.top
    dpi = user32.GetDpiForWindow(hwnd) if hasattr(user32, "GetDpiForWindow") else 96
    scale = dpi / 96.0
    print("hwnd=%s pixel=%dx%d dpi=%d scale=%.2f logical=%.0fx%.0f"
          % (hwnd, w, h, dpi, scale, w / scale, h / scale))
    return 0


def cmd_shot(pid, scale=2, name="ui-shot.png", crop=None):
    got = find_window(pid)
    if not got:
        print("window not found")
        return 1
    hwnd, rect = got
    ow, oh, pixels, w, h = grab(hwnd, rect, scale, crop)
    path = SHOT_DIR / name
    size = write_png(path, ow, oh, pixels)
    print("window %dx%d -> %dx%d, %d KB, %s" % (w, h, ow, oh, size // 1024, path))
    return 0


def cmd_max(pid):
    got = find_window(pid)
    if not got:
        print("window not found")
        return 1
    user32.ShowWindow(got[0], 3)
    time.sleep(1.2)
    return cmd_rect(pid)


def cmd_size(pid, w, h):
    got = find_window(pid)
    if not got:
        print("window not found")
        return 1
    hwnd, _rect = got
    SWP_NOMOVE, SWP_NOZORDER = 0x0002, 0x0004
    user32.SetWindowPos(hwnd, 0, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER)
    time.sleep(1.0)
    return cmd_rect(pid)


def cmd_key(pid, vk, times):
    got = find_window(pid)
    if not got:
        print("window not found")
        return 1
    hwnd, _rect = got
    for _ in range(times):
        user32.PostMessageW(hwnd, WM_KEYDOWN, vk, 1)
        user32.PostMessageW(hwnd, WM_KEYUP, vk, 0xC0000001)
        time.sleep(0.25)
    print("sent 0x%02X x%d" % (vk, times))
    return 0


def cmd_kill(pid):
    subprocess.run(["taskkill", "/PID", str(pid), "/F"], creationflags=0x08000000)
    print("killed %s" % pid)
    return 0


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd = argv[0]
    if cmd == "launch":
        return cmd_launch()
    if cmd == "rect" and len(argv) >= 2:
        return cmd_rect(int(argv[1]))
    if cmd == "shot" and len(argv) >= 2:
        crop = None
        if len(argv) > 4:
            crop = tuple(int(v) for v in argv[4].split(","))
        return cmd_shot(int(argv[1]), int(argv[2]) if len(argv) > 2 else 2,
                        argv[3] if len(argv) > 3 else "ui-shot.png", crop)
    if cmd == "size" and len(argv) >= 4:
        return cmd_size(int(argv[1]), int(argv[2]), int(argv[3]))
    if cmd == "max" and len(argv) >= 2:
        return cmd_max(int(argv[1]))
    if cmd == "key" and len(argv) >= 3:
        return cmd_key(int(argv[1]), int(argv[2], 16), int(argv[3]) if len(argv) > 3 else 1)
    if cmd == "kill" and len(argv) >= 2:
        return cmd_kill(int(argv[1]))
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
