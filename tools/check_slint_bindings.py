"""
Slint <-> Rust 回调绑定完整性门禁检查器

扫描 Slint 导出的顶级 Window 组件中声明的所有 callback，
校验其在 src/gui.rs 中是否有对应的 .on_<name>( 绑定，防止交互悬空断链。
"""

import argparse
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "src"


def extract_window_callbacks(slint_text: str) -> dict[str, list[tuple[str, int]]]:
    """
    提取 slint 文本中继承自 Window 的组件及其内部声明的 callback (包含行号)
    """
    results = {}
    lines = slint_text.splitlines()

    comp_pattern = re.compile(
        r"export\s+component\s+([a-zA-Z0-9_]+)\s+inherits\s+Window\s*\{"
    )
    cb_pattern = re.compile(r"^\s*callback\s+([a-zA-Z0-9_]+)\b")

    current_comp = None
    brace_depth = 0

    for idx, line in enumerate(lines, start=1):
        # 排除整行纯注释
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("/*"):
            continue

        if not current_comp:
            match = comp_pattern.search(line)
            if match:
                current_comp = match.group(1)
                brace_depth = line.count("{") - line.count("}")
                results[current_comp] = []
        else:
            brace_depth += line.count("{") - line.count("}")
            if brace_depth <= 0:
                current_comp = None
                brace_depth = 0
            else:
                cb_match = cb_pattern.search(line)
                if cb_match:
                    cb_name = cb_match.group(1)
                    results[current_comp].append((cb_name, idx))

    return results


def check_bindings(
    components: dict[str, list[tuple[str, int]]], rust_text: str
) -> tuple[int, list[str]]:
    """
    检查每个组件的回调是否在 rust_text 中被绑定 (.on_<name>()
    """
    total_callbacks = 0
    errors = []

    # 预先扫描所有 on_<name>( 调用
    bound_methods = set(re.findall(r"\.on_([a-zA-Z0-9_]+)\s*\(", rust_text))

    for comp_name, callbacks in components.items():
        for cb_name, line_no in callbacks:
            total_callbacks += 1
            if cb_name not in bound_methods:
                errors.append(
                    f"{comp_name} 回调未绑定: `{cb_name}` (Slint 第 {line_no} 行) -> 缺少 .on_{cb_name}(...)"
                )

    return total_callbacks, errors


def run_selfcheck():
    dummy_slint = """
    export component DummyWindow inherits Window {
        callback on_click();
        callback submitted(string);
        callback hanging_callback();
    }
    """
    dummy_rust = """
        dummy.on_on_click(move || {});
        dummy.on_submitted(move |s| {});
    """
    comps = extract_window_callbacks(dummy_slint)
    total, errs = check_bindings(comps, dummy_rust)
    assert total == 3, f"Selfcheck failed: expected 3 callbacks, got {total}"
    assert len(errs) == 1, f"Selfcheck failed: expected 1 error, got {len(errs)}"
    assert "hanging_callback" in errs[0], f"Selfcheck error mismatch: {errs[0]}"
    print("Selfcheck passed: 准确捕获 1 处未绑定悬空回调。")


def main():
    parser = argparse.ArgumentParser(description="Slint ↔ Rust 回调绑定完整性门禁")
    parser.add_argument("--selfcheck", action="store_true", help="运行检查器自检")
    args = parser.parse_args()

    if args.selfcheck:
        run_selfcheck()
        sys.exit(0)

    gui_rs_path = SRC_DIR / "gui.rs"
    if not gui_rs_path.is_file():
        print(f"错误: 未找到 {gui_rs_path}", file=sys.stderr)
        sys.exit(1)

    rust_text = gui_rs_path.read_text(encoding="utf-8")

    all_components = {}
    slint_files = [SRC_DIR / "cockpit.slint"]

    for sf in slint_files:
        if sf.is_file():
            text = sf.read_text(encoding="utf-8")
            comps = extract_window_callbacks(text)
            for c_name, cbs in comps.items():
                all_components[f"{sf.name}:{c_name}"] = cbs

    total, errors = check_bindings(all_components, rust_text)

    if errors:
        print(f"FAILED: 发现 {len(errors)} 处未绑定的 Slint 回调：", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        sys.exit(1)

    print(f"OK: Slint <-> Rust 回调绑定完整性检查通过，共 {total} 项回调已全部绑定：")
    for comp_key, cbs in sorted(all_components.items()):
        print(f"  - {comp_key}: {len(cbs)} 项回调全量在线")


if __name__ == "__main__":
    main()
