"""
Antigravity 待翻译生词提取工具

支持从解包目录或直接从 app.asar 中扫描 JS/HTML 文件，提取尚未收录至词典的 UI 英文短语，
并输出为待填写的 todo.json。
"""

import argparse
import json
import os
import re
import struct
import sys
from pathlib import Path

DEFAULT_DATA_DIR = Path(__file__).resolve().parent / "patch_builder" / "data"

KEY_PATTERNS = [
    re.compile(
        r'(?:label|title|description|placeholder|heading|tooltip|message|aria-label|ariaLabel)\s*:\s*["\']((?:(?![\\"\'])[^\\\n]|\\.){3,120})["\']',
        re.IGNORECASE,
    ),
    re.compile(
        r'<(?:button|span|p|h[1-6]|div|label|a)[^>]*>([A-Z][a-zA-Z0-9\s.,!?:\'()\-]{2,80})<\/',
        re.IGNORECASE,
    ),
]

CODE_FILTER = re.compile(
    r'(?:https?:\/\/|\.js|\.ts|\.json|\.css|\.png|\.svg|\.ico|\.asar|\/|\\|%s|\${|__|\(\))'
)


def load_existing_dictionaries(data_dir: Path) -> set[str]:
    known = set()
    for filename in ["ui.json", "core.json", "menu.json", "context_menu.json"]:
        path = data_dir / filename
        if path.is_file():
            try:
                with open(path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        known.update(k.strip() for k in data.keys())
            except Exception as e:
                print(f"[警告] 读取 {filename} 失败: {e}", file=sys.stderr)
    return known


def is_valid_candidate(text: str, known: set[str]) -> bool:
    s = text.strip()
    if len(s) < 3 or len(s) > 120:
        return False
    if s in known:
        return False
    if CODE_FILTER.search(s):
        return False
    if not re.match(r"^[A-Z][a-zA-Z0-9\s.,!?:\'\"()\-_–—]+$", s):
        return False
    words = s.split()
    if not words:
        return False
    if len(words) == 1 and s.isupper():
        return False
    return True


def scan_text(content: str, known: set[str], candidates: set[str]):
    for pattern in KEY_PATTERNS:
        for match in pattern.finditer(content):
            val = match.group(1).replace(r"\"", '"').replace(r"\'", "'").strip()
            if is_valid_candidate(val, known):
                candidates.add(val)


def scan_directory(dir_path: Path, known: set[str], candidates: set[str]) -> int:
    file_count = 0
    for root, _, files in os.walk(dir_path):
        for name in files:
            ext = os.path.splitext(name)[1].lower()
            if ext in [".js", ".html", ".htm", ".json"]:
                file_count += 1
                full = Path(root) / name
                try:
                    with open(full, "r", encoding="utf-8", errors="ignore") as f:
                        scan_text(f.read(), known, candidates)
                except Exception:
                    continue
    return file_count


def scan_asar(asar_path: Path, known: set[str], candidates: set[str]) -> int:
    file_count = 0
    with open(asar_path, "rb") as f:
        header_meta = f.read(16)
        if len(header_meta) < 16:
            return 0
        _, _, _, json_len = struct.unpack("<IIII", header_meta)
        header_json = f.read(json_len).decode("utf-8")
        header = json.loads(header_json)
        data_offset = 16 + json_len

        def walk(tree, prefix=""):
            for k, v in tree.items():
                p = f"{prefix}/{k}" if prefix else k
                if "files" in v:
                    yield from walk(v["files"], p)
                elif "size" in v and "offset" in v:
                    yield p, int(v["offset"]), int(v["size"])

        for rel, offset, size in walk(header.get("files", {})):
            ext = os.path.splitext(rel)[1].lower()
            if ext in [".js", ".html", ".htm"]:
                if rel.startswith("node_modules/"):
                    continue
                file_count += 1
                f.seek(data_offset + offset)
                content = f.read(size).decode("utf-8", errors="ignore")
                scan_text(content, known, candidates)

    return file_count


def auto_detect_target() -> Path | None:
    local_app_data = os.environ.get("LOCALAPPDATA", "")
    if local_app_data:
        p = Path(local_app_data) / "Programs" / "antigravity" / "resources" / "app.asar"
        if p.is_file():
            return p

    tmp = Path(__file__).resolve().parent.parent / ".tmp_extract"
    if tmp.is_dir():
        return tmp

    return None


def main():
    parser = argparse.ArgumentParser(description="提取 Antigravity 前端未汉化生词")
    parser.add_argument("target", nargs="?", help="目标目录路径或 app.asar 文件路径（缺省自动检测宿主）")
    parser.add_argument("-o", "--output", help="输出 json 路径（缺省为 data/todo.json）")
    parser.add_argument(
        "--data-dir",
        default=str(DEFAULT_DATA_DIR),
        help="已有词典所在目录",
    )
    args = parser.parse_args()

    data_dir = Path(args.data_dir).resolve()
    if not data_dir.is_dir():
        print(f"词典目录不存在: {data_dir}", file=sys.stderr)
        sys.exit(1)

    target_path = Path(args.target).resolve() if args.target else auto_detect_target()
    if not target_path or not target_path.exists():
        print("未指定有效目标，且未检测到宿主 app.asar 或 .tmp_extract 目录。", file=sys.stderr)
        print("用法: python tools/extract_todo.py <路径至 app.asar 或解包目录>", file=sys.stderr)
        sys.exit(1)

    output_path = Path(args.output).resolve() if args.output else data_dir / "todo.json"

    print(f"目标来源: {target_path}")
    print(f"词典基准: {data_dir}")

    known = load_existing_dictionaries(data_dir)
    print(f"已收录词条数: {len(known)}")

    candidates = set()
    if target_path.is_file() and target_path.suffix.lower() == ".asar":
        scanned_files = scan_asar(target_path, known, candidates)
    elif target_path.is_dir():
        scanned_files = scan_directory(target_path, known, candidates)
    elif target_path.is_file():
        scanned_files = 1
        with open(target_path, "r", encoding="utf-8", errors="ignore") as f:
            scan_text(f.read(), known, candidates)
    else:
        print(f"不支持的目标格式: {target_path}", file=sys.stderr)
        sys.exit(1)

    sorted_candidates = sorted(list(candidates))
    todo_dict = {item: "" for item in sorted_candidates}

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(todo_dict, f, ensure_ascii=False, indent=2)

    print(f"扫描文件数: {scanned_files}")
    print(f"提取新增候选生词: {len(sorted_candidates)} 条")
    print(f"输出文件: {output_path}")


if __name__ == "__main__":
    main()
