use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const HEADER_PREFIX_LEN: usize = 16;
pub const UNPACK_MARKER: &str = "chrome-devtools-mcp";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AsarNode {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unpacked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<BTreeMap<String, AsarNode>>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AsarHeader {
    files: BTreeMap<String, AsarNode>,
}

pub struct AsarArchive {
    header: AsarHeader,
    data_base: usize,
    data: Vec<u8>,
    unpacked_dir: Option<PathBuf>,
}

impl AsarArchive {
    pub fn open(path: &Path) -> Result<Self> {
        let data = fs::read(path)?;
        let mut archive = Self::from_bytes(data)?;
        archive.unpacked_dir = Some(unpacked_root(path));
        Ok(archive)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        if data.len() < HEADER_PREFIX_LEN {
            return Err(anyhow!("asar 长度不足 header 前缀"));
        }
        let json_len = u32::from_le_bytes(data[12..16].try_into()?) as usize;
        let header_end = HEADER_PREFIX_LEN + json_len;
        if data.len() < header_end {
            return Err(anyhow!("asar header 越界"));
        }
        let header: AsarHeader = serde_json::from_slice(&data[HEADER_PREFIX_LEN..header_end])?;
        let data_base = 8 + u32::from_le_bytes(data[4..8].try_into()?) as usize;
        Ok(Self {
            header,
            data_base,
            data,
            unpacked_dir: None,
        })
    }

    pub fn entry(&self, rel: &str) -> Option<&AsarNode> {
        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }
        let mut node = self.header.files.get(parts[0])?;
        for part in &parts[1..] {
            node = node.files.as_ref()?.get(*part)?;
        }
        Some(node)
    }

    pub fn read_file(&self, rel: &str) -> Option<&[u8]> {
        let node = self.entry(rel)?;
        if node.unpacked {
            return None;
        }
        let offset: usize = node.offset.as_ref()?.parse().ok()?;
        let start = self.data_base + offset;
        let end = start.checked_add(node.size as usize)?;
        if end > self.data.len() {
            return None;
        }
        Some(&self.data[start..end])
    }

    pub fn read_file_text(&self, rel: &str) -> Option<String> {
        String::from_utf8(self.read_file(rel)?.to_vec()).ok()
    }

    pub fn file_count(&self) -> usize {
        count_files(&self.header.files)
    }

    pub fn unpacked_count(&self) -> usize {
        count_unpacked(&self.header.files)
    }

    pub fn extract(&self, out_dir: &Path) -> Result<usize> {
        let mut written = 0usize;
        self.extract_dir(&self.header.files, out_dir, "", &mut written)?;
        Ok(written)
    }

    fn extract_dir(
        &self,
        files: &BTreeMap<String, AsarNode>,
        dir: &Path,
        rel_prefix: &str,
        written: &mut usize,
    ) -> Result<()> {
        fs::create_dir_all(dir)?;
        for (name, node) in files {
            let target = dir.join(name);
            let rel = if rel_prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel_prefix, name)
            };
            if let Some(children) = &node.files {
                self.extract_dir(children, &target, &rel, written)?;
                continue;
            }
            if node.unpacked {
                if let Some(root) = &self.unpacked_dir {
                    let from = root.join(&rel);
                    if from.is_file() {
                        if let Some(parent) = target.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::copy(&from, &target)?;
                        *written += 1;
                    }
                }
                continue;
            }
            let offset: usize = node
                .offset
                .as_ref()
                .and_then(|o| o.parse().ok())
                .ok_or_else(|| anyhow!("asar 条目缺 offset"))?;
            let start = self.data_base + offset;
            let end = start.checked_add(node.size as usize).ok_or_else(|| anyhow!("asar 条目越界"))?;
            if end > self.data.len() {
                return Err(anyhow!("asar 条目数据越界"));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, &self.data[start..end])?;
            *written += 1;
        }
        Ok(())
    }
}

fn count_files(files: &BTreeMap<String, AsarNode>) -> usize {
    let mut total = 0usize;
    for node in files.values() {
        if let Some(children) = &node.files {
            total += count_files(children);
        } else {
            total += 1;
        }
    }
    total
}

fn count_unpacked(files: &BTreeMap<String, AsarNode>) -> usize {
    let mut total = 0usize;
    for node in files.values() {
        if let Some(children) = &node.files {
            total += count_unpacked(children);
        } else if node.unpacked {
            total += 1;
        }
    }
    total
}

struct PackEntry {
    rel: String,
    abs: PathBuf,
    size: u64,
}

fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<PackEntry>) -> Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if path.is_dir() {
            collect_files(&path, &rel, out)?;
        } else {
            let size = fs::metadata(&path)?.len();
            out.push(PackEntry {
                rel,
                abs: path,
                size,
            });
        }
    }
    Ok(())
}

fn is_unpacked_rel(rel: &str) -> bool {
    rel.split('/').any(|seg| seg == UNPACK_MARKER)
}

fn insert_node(root: &mut BTreeMap<String, AsarNode>, rel: &str, node: AsarNode) {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut level = root;
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            level.insert(part.to_string(), node.clone());
            return;
        }
        let entry = level
            .entry(part.to_string())
            .or_insert_with(|| AsarNode {
                files: Some(BTreeMap::new()),
                ..Default::default()
            });
        level = entry.files.get_or_insert_with(BTreeMap::new);
    }
}

pub struct PackResult {
    pub file_count: usize,
    pub unpacked_count: usize,
}

pub fn pack(src_dir: &Path, out_path: &Path) -> Result<PackResult> {
    let mut entries = Vec::new();
    collect_files(src_dir, "", &mut entries)?;
    entries.sort_by(|a, b| a.rel.cmp(&b.rel));

    let mut root = BTreeMap::new();
    let mut packed: Vec<u8> = Vec::new();
    let mut unpacked_count = 0usize;

    for entry in &entries {
        if is_unpacked_rel(&entry.rel) {
            let dest = unpacked_root(out_path).join(&entry.rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&entry.abs, &dest)?;
            insert_node(
                &mut root,
                &entry.rel,
                AsarNode {
                    size: entry.size,
                    offset: Some("0".to_string()),
                    unpacked: true,
                    files: None,
                },
            );
            unpacked_count += 1;
            continue;
        }
        let content = fs::read(&entry.abs)?;
        let offset = packed.len();
        packed.extend_from_slice(&content);
        insert_node(
            &mut root,
            &entry.rel,
            AsarNode {
                size: content.len() as u64,
                offset: Some(offset.to_string()),
                unpacked: false,
                files: None,
            },
        );
    }

    let header = AsarHeader { files: root };
    let json = serde_json::to_vec(&header)?;
    let json_len = json.len() as u32;

    let mut out = Vec::with_capacity(HEADER_PREFIX_LEN + json.len() + packed.len());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(8 + json_len).to_le_bytes());
    out.extend_from_slice(&(4 + json_len).to_le_bytes());
    out.extend_from_slice(&json_len.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&packed);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, out)?;

    Ok(PackResult {
        file_count: entries.len(),
        unpacked_count,
    })
}

pub fn unpacked_root(asar_path: &Path) -> PathBuf {
    let mut name = asar_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".unpacked");
    match asar_path.parent() {
        Some(p) => p.join(name),
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_dir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("lander_asar_test_{}", tag));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn test_extract_and_read() {
        let base = work_dir("extract");
        let src = base.join("src");
        fs::create_dir_all(src.join("dist")).unwrap();
        fs::write(src.join("package.json"), b"{\"version\":\"9.9.9\"}").unwrap();
        fs::write(src.join("dist").join("main.js"), b"console.log(1);").unwrap();

        let asar_path = base.join("app.asar");
        let res = pack(&src, &asar_path).unwrap();
        assert_eq!(res.file_count, 2);

        let archive = AsarArchive::open(&asar_path).unwrap();
        assert_eq!(archive.file_count(), 2);
        assert_eq!(
            archive.read_file_text("package.json").unwrap(),
            "{\"version\":\"9.9.9\"}"
        );
        assert_eq!(
            archive.read_file_text("dist/main.js").unwrap(),
            "console.log(1);"
        );

        let out = base.join("out");
        let written = archive.extract(&out).unwrap();
        assert_eq!(written, 2);
        assert_eq!(
            fs::read_to_string(out.join("dist").join("main.js")).unwrap(),
            "console.log(1);"
        );
    }

    #[test]
    fn test_roundtrip_preserves_content() {
        let base = work_dir("roundtrip");
        let src = base.join("src");
        fs::create_dir_all(src.join("a").join("b")).unwrap();
        fs::write(src.join("a").join("b").join("deep.txt"), b"deep-value").unwrap();
        fs::write(src.join("top.bin"), vec![0u8, 1, 2, 250]).unwrap();

        let first = base.join("one.asar");
        pack(&src, &first).unwrap();

        let mid = base.join("mid");
        AsarArchive::open(&first).unwrap().extract(&mid).unwrap();

        let second = base.join("two.asar");
        pack(&mid, &second).unwrap();

        let a1 = AsarArchive::open(&first).unwrap();
        let a2 = AsarArchive::open(&second).unwrap();
        assert_eq!(a1.file_count(), a2.file_count());
        assert_eq!(
            a1.read_file_text("a/b/deep.txt").unwrap(),
            a2.read_file_text("a/b/deep.txt").unwrap()
        );
        assert_eq!(
            a1.read_file("top.bin").unwrap(),
            a2.read_file("top.bin").unwrap()
        );
    }

    #[test]
    fn test_unpacked_entries_are_excluded() {
        let base = work_dir("unpacked");
        let src = base.join("src");
        fs::create_dir_all(src.join("chrome-devtools-mcp")).unwrap();
        fs::write(src.join("keep.js"), b"keep").unwrap();
        fs::write(src.join("chrome-devtools-mcp").join("tool.js"), b"tool").unwrap();

        let asar_path = base.join("app.asar");
        let res = pack(&src, &asar_path).unwrap();
        assert_eq!(res.unpacked_count, 1);

        let archive = AsarArchive::open(&asar_path).unwrap();
        assert!(archive.read_file("chrome-devtools-mcp/tool.js").is_none());
        assert_eq!(archive.read_file_text("keep.js").unwrap(), "keep");
        assert!(unpacked_root(&asar_path)
            .join("chrome-devtools-mcp")
            .join("tool.js")
            .exists());
    }

    #[test]
    fn test_roundtrip_restores_unpacked_files() {
        let base = work_dir("unpacked_roundtrip");
        let src = base.join("src");
        fs::create_dir_all(src.join("dist")).unwrap();
        let mcp = src.join("node_modules").join(UNPACK_MARKER);
        fs::create_dir_all(&mcp).unwrap();
        fs::write(src.join("package.json"), b"{\"version\":\"1.2.3\"}").unwrap();
        fs::write(src.join("dist").join("main.js"), b"main-code").unwrap();
        fs::write(mcp.join("tool.js"), b"tool-code").unwrap();
        fs::write(mcp.join("LICENSE"), b"license-text").unwrap();

        let asar_path = base.join("app.asar");
        let res = pack(&src, &asar_path).unwrap();
        assert_eq!(res.file_count, 4);
        assert_eq!(res.unpacked_count, 2);

        let archive = AsarArchive::open(&asar_path).unwrap();
        assert_eq!(archive.file_count(), 4);

        let out = base.join("out");
        let written = archive.extract(&out).unwrap();
        assert_eq!(written, 4);
        assert_eq!(
            fs::read_to_string(out.join("node_modules").join(UNPACK_MARKER).join("tool.js")).unwrap(),
            "tool-code"
        );

        let rebuilt = base.join("rebuilt.asar");
        let again_res = pack(&out, &rebuilt).unwrap();
        assert_eq!(again_res.file_count, 4);
        assert_eq!(again_res.unpacked_count, 2);

        let again = AsarArchive::open(&rebuilt).unwrap();
        assert_eq!(again.file_count(), 4);
        assert_eq!(
            again.read_file_text("package.json").unwrap(),
            "{\"version\":\"1.2.3\"}"
        );
        assert_eq!(again.read_file_text("dist/main.js").unwrap(), "main-code");
    }

    #[test]
    fn test_header_layout_matches_spec() {
        let base = work_dir("layout");
        let src = base.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("x.txt"), b"abc").unwrap();

        let asar_path = base.join("app.asar");
        pack(&src, &asar_path).unwrap();

        let raw = fs::read(&asar_path).unwrap();
        let version = u32::from_le_bytes(raw[0..4].try_into().unwrap());
        let pickle_len = u32::from_le_bytes(raw[4..8].try_into().unwrap());
        let payload_len = u32::from_le_bytes(raw[8..12].try_into().unwrap());
        let json_len = u32::from_le_bytes(raw[12..16].try_into().unwrap());

        assert_eq!(version, 4);
        assert_eq!(pickle_len, 8 + json_len);
        assert_eq!(payload_len, 4 + json_len);
        assert_eq!(raw.len(), 16 + json_len as usize + 3);
    }
}
