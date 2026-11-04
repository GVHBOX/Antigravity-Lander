"""
Rust 模块分层与依赖单向性静态门禁检查器

扫描 src/*.rs 中的模块引用 (crate::*)，捍卫分层架构与单向依赖：
1. Layer 0 (底层纯工具/引擎): asar, localize, sound 强制零内部依赖；
2. Layer 1 (系统遥测采集): monitor 严禁依赖业务服务层与 UI 层；
3. Layer 2 (业务调度与服务): launcher, quota, proxy, session, widget_state 严禁反向依赖 UI 层 (gui)；
4. 全局拓扑无环 (DAG): 严禁双向依赖或任何形式的循环引用。
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "src"

LAYERS = {
    "asar": 0,
    "localize": 0,
    "sound": 0,
    "monitor": 1,
    "launcher": 2,
    "quota": 2,
    "proxy": 2,
    "session": 2,
    "widget_state": 2,
    "gui": 3,
    "main": 4,
}


def strip_string_literals(code: str) -> str:
    code = re.sub(r'r(#*)".*?"\1', '""', code, flags=re.DOTALL)
    code = re.sub(r'"(\\.|[^"\\])*"', '""', code)
    return code


def parse_module_dependencies(
    src_dir: Path, known_modules: set[str]
) -> dict[str, list[str]]:
    graph = {}
    pattern = re.compile(r"\bcrate::([a-zA-Z0-9_]+)\b")

    for rs_file in sorted(src_dir.glob("*.rs")):
        mod_name = rs_file.stem
        code = rs_file.read_text(encoding="utf-8")
        clean = strip_string_literals(code)
        raw_matches = pattern.findall(clean)
        deps = sorted({m for m in raw_matches if m in known_modules and m != mod_name})
        graph[mod_name] = deps

    return graph


def detect_cycles(graph: dict[str, list[str]]) -> list[list[str]]:
    visited = {}
    cycles = []

    def dfs(node: str, path: list[str]):
        visited[node] = 1
        path.append(node)

        for neighbor in graph.get(node, []):
            if neighbor not in visited:
                dfs(neighbor, path)
            elif visited[neighbor] == 1:
                cycle_start = path.index(neighbor)
                cycles.append(path[cycle_start:] + [neighbor])

        path.pop()
        visited[node] = 2

    for n in graph:
        if n not in visited:
            dfs(n, [])

    return cycles


def verify_architecture_boundaries(
    graph: dict[str, list[str]], layers: dict[str, int]
) -> list[str]:
    violations = []

    ui_modules = {m for m, l in layers.items() if l == 3}

    for mod, deps in graph.items():
        mod_layer = layers.get(mod, 99)

        if mod_layer == 0 and deps:
            violations.append(
                f"Layer 0 纯净性违规: `{mod}.rs` 必须为零内部依赖模块，但引用了: {deps}"
            )

        if mod not in ui_modules and mod_layer < 3:
            for u in ui_modules:
                if u in deps:
                    violations.append(
                        f"UI 反向穿透违规: 非 UI 模块 `{mod}.rs` 严禁反向依赖 UI 模块 `{u}.rs`"
                    )

        if mod_layer == 1:
            illegal = [d for d in deps if layers.get(d, 99) > 1]
            if illegal:
                violations.append(
                    f"Layer 1 向上越界违规: `{mod}.rs` 严禁依赖上层模块: {illegal}"
                )

        if mod_layer == 2:
            illegal = [d for d in deps if layers.get(d, 99) > 2]
            if illegal:
                violations.append(
                    f"Layer 2 向上越界违规: `{mod}.rs` 严禁依赖上层模块: {illegal}"
                )

    cycles = detect_cycles(graph)
    for c in cycles:
        chain = " -> ".join(c)
        violations.append(f"循环依赖违规 (Cycle Detected): {chain}")

    return violations


def run_selfcheck():
    dummy_layers = {
        "core_tool": 0,
        "sensor": 1,
        "biz_logic": 2,
        "biz_other": 2,
        "ui": 3,
        "entry": 4,
    }

    # 1. 正常拓扑
    valid_graph = {
        "core_tool": [],
        "sensor": ["core_tool"],
        "biz_logic": ["core_tool", "sensor"],
        "biz_other": ["biz_logic"],
        "ui": ["biz_logic", "biz_other"],
        "entry": ["ui"],
    }
    v_clean = verify_architecture_boundaries(valid_graph, dummy_layers)
    assert (
        len(v_clean) == 0
    ), f"Selfcheck failed: expected 0 violations, got {len(v_clean)}"

    # 2. Layer 0 污染
    l0_polluted = {"core_tool": ["sensor"]}
    v_l0 = verify_architecture_boundaries(l0_polluted, dummy_layers)
    assert any("Layer 0 纯净性违规" in v for v in v_l0), "Selfcheck failed on Layer 0 check"

    # 3. UI 反向穿透
    ui_breach = {"biz_logic": ["ui"]}
    v_ui = verify_architecture_boundaries(ui_breach, dummy_layers)
    assert any("UI 反向穿透违规" in v for v in v_ui), "Selfcheck failed on UI breach check"

    # 4. 循环依赖
    cycle_graph = {
        "biz_logic": ["biz_other"],
        "biz_other": ["biz_logic"],
    }
    v_cycle = verify_architecture_boundaries(cycle_graph, dummy_layers)
    assert any("循环依赖违规" in v for v in v_cycle), "Selfcheck failed on Cycle check"

    print("Selfcheck passed: 架构分层、UI单向性与拓扑无环门禁断言全部校准通过。")


def main():
    parser = argparse.ArgumentParser(
        description="Rust 模块分层与依赖单向性静态门禁"
    )
    parser.add_argument("--json", action="store_true", help="输出完整依赖拓扑 JSON")
    parser.add_argument("--selfcheck", action="store_true", help="运行检查器自检断言")
    args = parser.parse_args()

    if args.selfcheck:
        run_selfcheck()
        sys.exit(0)

    if not SRC_DIR.is_dir():
        print(f"错误: 未找到 src 目录: {SRC_DIR}", file=sys.stderr)
        sys.exit(1)

    known_modules = set(LAYERS.keys())
    graph = parse_module_dependencies(SRC_DIR, known_modules)
    violations = verify_architecture_boundaries(graph, LAYERS)

    if args.json:
        payload = {
            "layers": LAYERS,
            "dependency_graph": graph,
            "violations": violations,
            "is_valid": len(violations) == 0,
        }
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print("=== Antigravity Lander 架构分层与依赖拓扑巡检报告 ===")
        for mod, deps in sorted(graph.items()):
            layer = LAYERS.get(mod, "?")
            dep_str = ", ".join(deps) if deps else "(无内部依赖)"
            print(f"  [L{layer}] {mod:<14} -> {dep_str}")

        print("")
        if violations:
            print(
                f"FAILED: 发现 {len(violations)} 项架构分层越界违规：", file=sys.stderr
            )
            for v in violations:
                print(f"  ! {v}", file=sys.stderr)
            sys.exit(1)
        else:
            print("OK: 架构分层检查通过，零反向引用、零环路依赖。")


if __name__ == "__main__":
    main()
