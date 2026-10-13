"""铁律 2 机器强制检查：界面不加说明字。

只写「它是什么」和「出了什么问题」，不写「怎么用」。禁止解释性与引导性文字。
示范格式用 placeholder，不加说明行。

扫描范围：ui/*.slint 的界面文案 + src/*.rs 里给人看的字符串（toast / 状态条 / 标签）。
诊断日志（append_log / add_log 的内容）不算界面，已排除。

注意：这是关键词黑名单，只能挡住已知说法。新增文案仍要按铁律 2 人工过一遍。

用法：
    python tools/check_front_hygiene.py
    python tools/check_front_hygiene.py --selfcheck
"""

from __future__ import annotations

import sys
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ["src/**/*.slint", "src/**/*.rs"]

BANNED_WORDS = (
    "点击", "请选择", "您可以", "建议", "试试", "请注意",
    "使用方法", "该字段", "点击这里", "请先", "请再", "请把",
    "请确认", "请检查", "请重新", "换一个", "先启用", "然后在",
    "即可", "发给我", "粘贴到", "留空用", "怎么用", "按 Space",
    "按 [Space]", "按 [K]", "按 [Q]", "无需担心", "不用担心",
    "物理级", "纯机器码", "神级", "极致",
    "已调用系统接口", "不受任何影响", "完全脱钩", "独立派生", "彻底退出",
    "回收语言服务", "脱机启动", "脱机运行", "原生继承",
)

LOG_MARKERS = ("append_log(", "add_log(", "log_content")


def scan_hygiene(root: Path) -> list[str]:
    errors = []
    for pattern in TARGETS:
        for p in sorted(root.glob(pattern)):
            text = p.read_text(encoding="utf-8", errors="replace")
            for line_no, line in enumerate(text.splitlines(), 1):
                if any(m in line for m in LOG_MARKERS):
                    continue
                for banned in BANNED_WORDS:
                    if banned in line:
                        errors.append(
                            f"{p.name}:{line_no} 命中禁止引导词 '{banned}': {line.strip()[:80]}"
                        )
    return errors


def selfcheck() -> int:
    clean_sample = 'value: "未运行"; ui.set_toast_message("分身未在运行。".into());'
    dirty_samples = [
        'Text { text: "点击这里启动分身"; }',
        'ui.set_toast_message("请先启动分身，然后再按 K".into());',
        'value: "留空用默认端口";',
    ]
    for banned in BANNED_WORDS:
        if banned in clean_sample:
            print(f"Selfcheck FAILED: clean sample falsely hit banned word {banned}")
            return 1
    found = sum(1 for d in dirty_samples if any(b in d for b in BANNED_WORDS))
    if found != len(dirty_samples):
        print(f"Selfcheck FAILED: dirty samples not all caught ({found}/{len(dirty_samples)})")
        return 1
    print("Selfcheck PASS: 铁律 2 扫描器校准通过")
    return 0


def main(argv: list[str]) -> int:
    if "--selfcheck" in argv:
        return selfcheck()
    errors = scan_hygiene(ROOT)
    if errors:
        print(f"FAILED: 发现 {len(errors)} 处铁律 2 违规:")
        for err in errors:
            print("  ", err)
        return 1
    print("OK: 铁律 2 界面检查通过，零违规")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
