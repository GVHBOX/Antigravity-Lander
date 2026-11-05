"""
Antigravity Lander 桌面实机原画截图工具

利用 Win32 GDI 与用户输入桌面句柄 (OpenInputDesktop + SetThreadDesktop)，
对 Antigravity Lander 真实桌面视口、悬浮球或完整桌面进行 1:1 物理像素原画捕获并落盘 .scratch/shots/。
"""

import argparse
import ctypes
import os
import sys
import time
from ctypes import wintypes
from datetime import datetime
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SHOTS_DIR = ROOT / ".scratch"

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
kernel32 = ctypes.windll.kernel32

SRCCOPY = 0x00CC0020
CAPTUREBLT = 0x40000000


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD),
        ("biWidth", wintypes.LONG),
        ("biHeight", wintypes.LONG),
        ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", wintypes.LONG),
        ("biYPelsPerMeter", wintypes.LONG),
        ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER)]


def attach_input_desktop():
    """附加当前线程到用户的交互桌面"""
    h_desk = user32.OpenInputDesktop(0, False, 0x01FF)
    if not h_desk:
        return False
    return bool(user32.SetThreadDesktop(h_desk))


def wake_lander_window():
    """通过激活事件唤醒 Lander 窗口，走 Slint 内部安全显示与增量重绘流程"""
    EVENT_MODIFY_STATE = 0x0002
    h_event = kernel32.OpenEventW(
        EVENT_MODIFY_STATE, False, "Local\\AntigravityCockpit.Activate"
    )
    if h_event:
        kernel32.SetEvent(h_event)
        kernel32.CloseHandle(h_event)
        time.sleep(0.3)


def find_lander_processes() -> list[int]:
    """寻找正在运行的 Antigravity-Lander 进程 PID"""
    pids = []
    h_snapshot = kernel32.CreateToolhelp32Snapshot(0x00000002, 0)
    if h_snapshot == -1:
        return pids

    class PROCESSENTRY32(ctypes.Structure):
        _fields_ = [
            ("dwSize", wintypes.DWORD),
            ("cntUsage", wintypes.DWORD),
            ("th32ProcessID", wintypes.DWORD),
            ("th32DefaultHeapID", ctypes.c_void_p),
            ("th32ModuleID", wintypes.DWORD),
            ("cntThreads", wintypes.DWORD),
            ("th32ParentProcessID", wintypes.DWORD),
            ("pcPriClassBase", wintypes.LONG),
            ("dwFlags", wintypes.DWORD),
            ("szExeFile", ctypes.c_wchar * 260),
        ]

    pe = PROCESSENTRY32()
    pe.dwSize = ctypes.sizeof(PROCESSENTRY32)
    if kernel32.Process32FirstW(h_snapshot, ctypes.byref(pe)):
        while True:
            name = pe.szExeFile.lower()
            if "antigravity-lander" in name or "lander" in name:
                pids.append(pe.th32ProcessID)
            if not kernel32.Process32NextW(h_snapshot, ctypes.byref(pe)):
                break
    kernel32.CloseHandle(h_snapshot)
    return pids


def locate_lander_windows(pids: list[int]):
    """定位主视口窗口与悬浮球窗口的 HWND 与坐标"""
    main_win = None
    widget_win = None
    target_desk_handle = None

    def scan_desktop(h_desk):
        nonlocal main_win, widget_win

        def enum_desk_proc(hwnd, lparam):
            nonlocal main_win, widget_win
            pid = wintypes.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            if pid.value in pids:
                length = user32.GetWindowTextLengthW(hwnd)
                buff = ctypes.create_unicode_buffer(length + 1)
                user32.GetWindowTextW(hwnd, buff, length + 1)
                class_buff = ctypes.create_unicode_buffer(64)
                user32.GetClassNameW(hwnd, class_buff, 64)

                rect = wintypes.RECT()
                user32.GetWindowRect(hwnd, ctypes.byref(rect))
                w = rect.right - rect.left
                h = rect.bottom - rect.top

                if class_buff.value == "Window Class":
                    if "Lander" in buff.value:
                        main_win = (hwnd, (rect.left, rect.top, w, h), buff.value)
                    elif w < 800 and w > 0 and h > 0:
                        widget_win = (hwnd, (rect.left, rect.top, w, h), "FloatingWidget")
            return True

        DESKENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        user32.EnumDesktopWindows(h_desk, DESKENUMPROC(enum_desk_proc), 0)

    h_input = user32.OpenInputDesktop(0, False, 0x01FF)
    if h_input:
        scan_desktop(h_input)
        if main_win or widget_win:
            target_desk_handle = h_input
        else:
            user32.CloseDesktop(h_input)

    if not (main_win or widget_win):
        hwinsta = user32.GetProcessWindowStation()
        if hwinsta:
            def enum_desk(dname, dlp):
                nonlocal target_desk_handle
                if main_win or widget_win:
                    return True
                hd = user32.OpenDesktopW(dname, 0, False, 0x01FF)
                if hd:
                    scan_desktop(hd)
                    if main_win or widget_win:
                        target_desk_handle = hd
                    else:
                        user32.CloseDesktop(hd)
                return True

            CB_DESK = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.LPCWSTR, wintypes.LPARAM)
            user32.EnumDesktopsW(hwinsta, CB_DESK(enum_desk), 0)

    if target_desk_handle:
        user32.SetThreadDesktop(target_desk_handle)

    return main_win, widget_win


def capture_rect(x: int, y: int, w: int, h: int) -> Image.Image:
    """使用 GDI BitBlt 抓取指定物理矩形区域的原画"""
    hdc_screen = user32.GetDC(0)
    hdc_mem = gdi32.CreateCompatibleDC(hdc_screen)
    hbm = gdi32.CreateCompatibleBitmap(hdc_screen, w, h)
    gdi32.SelectObject(hdc_mem, hbm)

    gdi32.BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, x, y, SRCCOPY | CAPTUREBLT)

    bmi = BITMAPINFO()
    bmi.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    bmi.bmiHeader.biWidth = w
    bmi.bmiHeader.biHeight = -h
    bmi.bmiHeader.biPlanes = 1
    bmi.bmiHeader.biBitCount = 32
    bmi.bmiHeader.biCompression = 0

    buf = (ctypes.c_byte * (w * h * 4))()
    gdi32.GetDIBits(hdc_mem, hbm, 0, h, ctypes.byref(buf), ctypes.byref(bmi), 0)

    img = Image.frombuffer("RGB", (w, h), bytes(buf), "raw", "BGRX", 0, 1)

    gdi32.DeleteObject(hbm)
    gdi32.DeleteDC(hdc_mem)
    user32.ReleaseDC(0, hdc_screen)

    return img


def run_selfcheck():
    """自检 GDI 屏幕抓取通道"""
    if not attach_input_desktop():
        print("Selfcheck FAILED: 无法附加到输入桌面。", file=sys.stderr)
        sys.exit(1)
    w = user32.GetSystemMetrics(0)
    h = user32.GetSystemMetrics(1)
    if w <= 0 or h <= 0:
        print("Selfcheck FAILED: 屏幕分辨率非法。", file=sys.stderr)
        sys.exit(1)
    img = capture_rect(0, 0, min(100, w), min(100, h))
    assert img.size == (min(100, w), min(100, h))
    print(f"Selfcheck passed: 桌面 GDI 抓取正常，物理屏幕分辨率 {w}x{h}。")


def main():
    parser = argparse.ArgumentParser(description="Antigravity Lander 桌面实机原画截图工具")
    parser.add_argument(
        "--target",
        choices=["window", "widget", "full"],
        default="window",
        help="截取目标: window(主视口), widget(悬浮球), full(全桌面)",
    )
    parser.add_argument("-o", "--output", help="输出图片路径 (缺省自动保存至 .scratch/)")
    parser.add_argument("--selfcheck", action="store_true", help="校验抓取通道")
    args = parser.parse_args()

    if args.selfcheck:
        run_selfcheck()
        sys.exit(0)

    if not attach_input_desktop():
        print("错误: 无法附加到输入桌面窗口站。", file=sys.stderr)
        sys.exit(1)

    pids = find_lander_processes()
    if not pids and args.target != "full":
        print("错误: 未检测到运行中的 Antigravity-Lander 进程。", file=sys.stderr)
        sys.exit(1)

    SHOTS_DIR.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

    if args.target == "full":
        w = user32.GetSystemMetrics(0)
        h = user32.GetSystemMetrics(1)
        img = capture_rect(0, 0, w, h)
        out_path = Path(args.output) if args.output else SHOTS_DIR / f"desktop_full_{timestamp}.png"
        img.save(out_path)
        print(f"OK: 桌面全屏原画已捕获 ({w}x{h}) -> {out_path}")
        return

    # 定位窗口
    main_win, widget_win = locate_lander_windows(pids)

    # 若主窗口最小化 (坐标通常为 -32000)，通过事件自动唤醒
    if args.target == "window":
        if not main_win or main_win[1][0] <= -10000:
            print("主窗口未激活或处于最小化，正在通过激活事件唤醒...")
            wake_lander_window()
            main_win, widget_win = locate_lander_windows(pids)

        if not main_win:
            print("错误: 未能定位到 Lander 主视口窗口。", file=sys.stderr)
            sys.exit(1)

        x, y, w, h = main_win[1]
        img = capture_rect(x, y, w, h)
        out_path = Path(args.output) if args.output else SHOTS_DIR / f"cockpit_live_{timestamp}.png"
        img.save(out_path)
        print(f"OK: Lander 主视口原画已捕获 (PID: {pids[0]}, 物理尺寸: {w}x{h}, 坐标: {x},{y})")
        print(f"输出文件: {out_path}")

    elif args.target == "widget":
        if not widget_win:
            print("错误: 未能定位到悬浮球窗口 (可能处于隐藏模式)。", file=sys.stderr)
            sys.exit(1)

        x, y, w, h = widget_win[1]
        img = capture_rect(x, y, w, h)
        out_path = Path(args.output) if args.output else SHOTS_DIR / f"widget_live_{timestamp}.png"
        img.save(out_path)
        print(f"OK: 桌面悬浮球原画已捕获 (物理尺寸: {w}x{h}, 坐标: {x},{y})")
        print(f"输出文件: {out_path}")


if __name__ == "__main__":
    main()
