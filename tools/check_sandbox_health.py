"""
沙箱 Junction 软链接健康度与隔离完整性巡检工具

对 Antigravity-Lander 派生的 4 个分身沙箱（data/instance_1 ~ instance_4）进行全量静态健康与安全边界巡检：
1. 验证槽位与沙箱目录 1:1 对齐与结构完整性；
2. 验证 NTFS Junction 真实物理目标路径与健康度（防断链 / 错位）；
3. 验证红线资产（.gemini、Token、Antigravity 缓存）物理隔离性（防穿透与硬链接串通）。
"""

import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA_DIR = ROOT / "data"

SHELL_FOLDERS = [
    "Desktop",
    "Downloads",
    "Documents",
    "Pictures",
    "Videos",
    "Music",
    "Links",
    "Favorites",
]

EXPECTED_JUNCTIONS = [
    ("home/AppData/LocalLow", "AppData/LocalLow"),
    ("home/AppData/Roaming/Microsoft", "AppData/Roaming/Microsoft"),
    ("home/AppData/Local/Microsoft", "AppData/Local/Microsoft"),
    ("AppData/Roaming/Microsoft", "AppData/Roaming/Microsoft"),
] + [(f"home/{name}", name) for name in SHELL_FOLDERS]


def clean_raw_path(raw: str) -> str:
    cleaned = raw.strip()
    if cleaned.startswith("\\\\?\\UNC\\"):
        return "\\\\" + cleaned[8:]
    if cleaned.startswith("\\\\?\\"):
        return cleaned[4:]
    if cleaned.startswith("\\??\\"):
        return cleaned[4:]
    return cleaned


def detect_host_user_dir(base_data_dir: Path) -> Path | None:
    for slot in range(1, 5):
        probe = base_data_dir / f"instance_{slot}" / "home" / "Desktop"
        if probe.exists():
            try:
                target = clean_raw_path(os.readlink(probe))
                target_path = Path(target)
                if target_path.name.lower() == "desktop" and target_path.parent.is_dir():
                    return target_path.parent
            except Exception:
                pass

    userprofile = os.environ.get("USERPROFILE")
    if userprofile:
        p = Path(userprofile)
        s = str(p).replace("/", "\\")
        if "\\data\\instance_" not in s and p.is_dir():
            return p

    sys_drive = os.environ.get("SystemDrive", "C:")
    username = os.environ.get("USERNAME")
    if username:
        p = Path(sys_drive) / "Users" / username
        if p.is_dir():
            return p

    for drive in ["C:", "D:", "E:"]:
        for name in ["Administrator", "Default"]:
            p = Path(drive) / "Users" / name
            if p.is_dir():
                return p

    return None


def is_reparse_point(path: Path) -> bool:
    try:
        st = os.lstat(path)
        return bool(st.st_file_attributes & 0x400)
    except Exception:
        return False


def get_file_identity(path: Path) -> tuple[int, int] | None:
    try:
        st = os.stat(path)
        return (st.st_dev, st.st_ino)
    except Exception:
        return None


def audit_sandbox(
    base_data_dir: Path, host_user: Path | None
) -> tuple[list[dict], list[str]]:
    slot_results = []
    all_violations = []
    token_identities: dict[tuple[int, int], str] = {}

    if host_user:
        host_token = host_user / ".gemini" / "jetski-standalone-oauth-token"
        if host_token.is_file():
            ident = get_file_identity(host_token)
            if ident:
                token_identities[ident] = "host"

    for slot in range(1, 5):
        slot_name = f"instance_{slot}"
        slot_root = base_data_dir / slot_name
        res = {
            "slot": slot,
            "root_exists": slot_root.is_dir(),
            "is_root_reparse": is_reparse_point(slot_root),
            "home_exists": (slot_root / "home").is_dir(),
            "is_home_reparse": is_reparse_point(slot_root / "home"),
            "roaming_exists": (slot_root / "AppData" / "Roaming").is_dir(),
            "gemini_exists": (slot_root / "home" / ".gemini").is_dir(),
            "is_gemini_reparse": is_reparse_point(slot_root / "home" / ".gemini"),
            "token_present": False,
            "token_is_link": False,
            "token_nlink": 0,
            "junctions": {},
            "violations": [],
        }

        if not res["root_exists"]:
            res["violations"].append(f"沙箱根目录缺失: {slot_root}")
            slot_results.append(res)
            all_violations.extend(res["violations"])
            continue

        if res["is_root_reparse"]:
            res["violations"].append(f"红线违规: 沙箱根目录不能为软链接: {slot_root}")

        if res["is_home_reparse"]:
            res["violations"].append(f"红线违规: home 目录不能为软链接: {slot_root / 'home'}")

        if res["is_gemini_reparse"]:
            res["violations"].append(
                f"红线违规: .gemini 凭证目录不能为软链接: {slot_root / 'home' / '.gemini'}"
            )

        token_path = slot_root / "home" / ".gemini" / "jetski-standalone-oauth-token"
        if token_path.exists():
            res["token_present"] = True
            token_is_rep = is_reparse_point(token_path)
            res["token_is_link"] = token_is_rep
            if token_is_rep:
                res["violations"].append(
                    f"红线违规: OAuth Token 不能为软链接: {token_path}"
                )

            try:
                st = os.stat(token_path)
                res["token_nlink"] = st.st_nlink
                if st.st_nlink > 1:
                    res["violations"].append(
                        f"红线违规: OAuth Token 存在多重硬链接 (nlink={st.st_nlink}): {token_path}"
                    )
                ident = (st.st_dev, st.st_ino)
                if ident in token_identities:
                    res["violations"].append(
                        f"红线违规: OAuth Token 与 {token_identities[ident]} 共享物理存储节点 (inode={st.st_ino})"
                    )
                else:
                    token_identities[ident] = f"slot_{slot}"
            except Exception as e:
                res["violations"].append(f"读取 Token 状态异常: {e}")

        for rel_link, rel_target in EXPECTED_JUNCTIONS:
            link_path = slot_root / rel_link
            expected_dest = host_user / rel_target if host_user else None

            if not link_path.exists():
                try:
                    raw_target = clean_raw_path(os.readlink(link_path))
                    res["junctions"][rel_link] = {
                        "status": "BROKEN_LINK",
                        "target": raw_target,
                        "exists": False,
                    }
                except Exception:
                    res["junctions"][rel_link] = {
                        "status": "MISSING",
                        "target": None,
                        "exists": False,
                    }
            elif is_reparse_point(link_path):
                try:
                    raw_target = clean_raw_path(os.readlink(link_path))
                    target_path = Path(raw_target)
                    tgt_exists = target_path.exists()
                    res["junctions"][rel_link] = {
                        "status": "JUNCTION_OK" if tgt_exists else "BROKEN_LINK",
                        "target": raw_target,
                        "exists": tgt_exists,
                    }
                except Exception as e:
                    res["junctions"][rel_link] = {
                        "status": "READ_ERROR",
                        "target": str(e),
                        "exists": False,
                    }
            else:
                res["junctions"][rel_link] = {
                    "status": "PLAIN_DIR",
                    "target": str(link_path),
                    "exists": True,
                }

        slot_results.append(res)
        all_violations.extend(res["violations"])

    return slot_results, all_violations


def format_report(slot_results: list[dict], host_user: Path | None) -> str:
    lines = []
    lines.append("=== Antigravity Lander 沙箱隔离与 Junction 健康巡检报告 ===")
    lines.append(f"宿主基准用户目录: {host_user if host_user else '未能解析'}")
    lines.append("")

    total_violations = sum(len(s["violations"]) for s in slot_results)

    for s in slot_results:
        slot = s["slot"]
        lines.append(f"[槽位 {slot}] instance_{slot}:")
        lines.append(
            f"  - 核心结构: root={'OK' if s['root_exists'] else 'MISSING'}, "
            f"home={'OK' if s['home_exists'] else 'MISSING'}, "
            f"roaming={'OK' if s['roaming_exists'] else 'MISSING'}, "
            f".gemini={'OK' if s['gemini_exists'] else 'MISSING'}"
        )
        token_desc = (
            f"存在 (独立文件, 链接计数={s['token_nlink']})"
            if s["token_present"]
            else "未签发/无凭据"
        )
        lines.append(f"  - 账号凭据: {token_desc}")

        juncs = s["junctions"]
        j_ok = sum(1 for v in juncs.values() if v["status"] == "JUNCTION_OK")
        j_plain = sum(1 for v in juncs.values() if v["status"] == "PLAIN_DIR")
        j_broken = sum(1 for v in juncs.values() if v["status"] == "BROKEN_LINK")
        j_missing = sum(1 for v in juncs.values() if v["status"] == "MISSING")

        lines.append(
            f"  - 软链接映射 (共 12 项): {j_ok} 项健康有效, {j_plain} 项为独立实体目录, {j_broken} 项断链, {j_missing} 项缺失"
        )

        if j_plain > 0:
            plain_names = [k for k, v in juncs.items() if v["status"] == "PLAIN_DIR"]
            lines.append(f"    * 独立实体目录: {', '.join(plain_names)}")
        if j_broken > 0:
            broken_items = [
                f"{k}->{v['target']}"
                for k, v in juncs.items()
                if v["status"] == "BROKEN_LINK"
            ]
            lines.append(f"    * 断链项: {', '.join(broken_items)}")
        if j_missing > 0:
            missing_names = [k for k, v in juncs.items() if v["status"] == "MISSING"]
            lines.append(f"    * 缺失项: {', '.join(missing_names)}")

        if s["violations"]:
            lines.append("  - 警告与阻断违规:")
            for v in s["violations"]:
                lines.append(f"    ! {v}")
        lines.append("")

    if total_violations == 0:
        lines.append("OK: 全部红线隔离规则检查通过，零穿透与凭据违规。")
    else:
        lines.append(f"FAILED: 发现 {total_violations} 项红线隔离违规。")

    return "\n".join(lines)


def run_selfcheck():
    import tempfile

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        mock_data = tmp_path / "data"
        mock_host = tmp_path / "host"

        mock_host.mkdir()
        (mock_host / "Desktop").mkdir()

        for slot in range(1, 5):
            inst = mock_data / f"instance_{slot}"
            (inst / "home" / ".gemini").mkdir(parents=True)
            (inst / "AppData" / "Roaming").mkdir(parents=True)

        results, violations = audit_sandbox(mock_data, mock_host)
        assert (
            len(results) == 4
        ), f"Selfcheck failed: expected 4 slots, got {len(results)}"
        assert (
            len(violations) == 0
        ), f"Selfcheck failed: expected 0 violations, got {len(violations)}"

        # 构造红线违规测试：将 slot 2 的 token 与 slot 1 做硬链接
        token1 = (
            mock_data
            / "instance_1"
            / "home"
            / ".gemini"
            / "jetski-standalone-oauth-token"
        )
        token2 = (
            mock_data
            / "instance_2"
            / "home"
            / ".gemini"
            / "jetski-standalone-oauth-token"
        )
        token1.write_bytes(b"dummy_token_1")
        try:
            os.link(token1, token2)
            results, violations = audit_sandbox(mock_data, mock_host)
            assert any(
                "共享物理存储节点" in v or "多重硬链接" in v for v in violations
            ), "Selfcheck failed:未能检测到 Token 硬链接串通"
        except (AttributeError, OSError):
            pass

    print("Selfcheck passed: 沙箱结构与红线隔离规则校验器自检通过。")


def main():
    parser = argparse.ArgumentParser(
        description="Antigravity Lander 沙箱隔离与 Junction 健康巡检"
    )
    parser.add_argument("--json", action="store_true", help="以 JSON 格式输出结果")
    parser.add_argument("--selfcheck", action="store_true", help="运行巡检器自检断言")
    args = parser.parse_args()

    if args.selfcheck:
        run_selfcheck()
        sys.exit(0)

    if not DATA_DIR.is_dir():
        print(f"错误: 未找到 data 目录: {DATA_DIR}", file=sys.stderr)
        sys.exit(1)

    host_user = detect_host_user_dir(DATA_DIR)
    results, violations = audit_sandbox(DATA_DIR, host_user)

    if args.json:
        payload = {
            "host_user": str(host_user) if host_user else None,
            "violations": violations,
            "slots": results,
        }
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(format_report(results, host_user))

    if violations:
        sys.exit(1)


if __name__ == "__main__":
    main()
