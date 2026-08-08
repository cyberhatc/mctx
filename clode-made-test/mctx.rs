//! .mctx (Memory Context) reader/writer — no external crates.
//! Mirrors the logic of mctx.c / mctx.hpp (index + seek-read + reindex-on-write).
//! NOTE: written and reasoned through carefully, but not compiled in this
//! environment (no rustc available here) — review before trusting in prod.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub tier: String, // "!fixed" | "!durable" | "!volatile"
    pub version: u32,
    pub offset: u64,
}

pub struct Store {
    path: String,
    index: Vec<Section>,
}

impl Store {
    pub fn open(path: &str) -> io::Result<Self> {
        let mut s = Store { path: path.to_string(), index: Vec::new() };
        s.reload()?;
        Ok(s)
    }

    pub fn index(&self) -> &[Section] {
        &self.index
    }

    /// Parse only the %%INDEX ... %%END-INDEX block, not the whole file.
    pub fn reload(&mut self) -> io::Result<()> {
        let content = fs::read_to_string(&self.path)?;
        self.index.clear();
        let mut in_index = false;
        for line in content.lines() {
            if line.starts_with("%%INDEX") {
                in_index = true;
                continue;
            }
            if line.starts_with("%%END-INDEX") {
                break;
            }
            if !in_index {
                continue;
            }
            // "name:tier:vN:offset"
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() == 4 {
                let version = parts[2].trim_start_matches('v').parse().unwrap_or(1);
                let offset = parts[3].parse().unwrap_or(0);
                self.index.push(Section {
                    name: parts[0].to_string(),
                    tier: parts[1].to_string(),
                    version,
                    offset,
                });
            }
        }
        Ok(())
    }

    /// Seek straight to the section's byte offset and read until "%%END" —
    /// never reads the rest of the file.
    pub fn read(&self, name: &str) -> io::Result<String> {
        let entry = self
            .index
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "section not in index"))?;

        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(entry.offset))?;
        let mut rest = String::new();
        f.read_to_string(&mut rest)?;

        let mut lines = rest.lines();
        lines.next(); // skip the "%%@name !tier v:N" marker line itself

        let mut body = String::new();
        for line in lines {
            if line.starts_with("%%END") {
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        Ok(body)
    }

    /// Update a section's body (bumping its version) or create it if it
    /// doesn't exist yet, then rebuild the index so offsets stay correct.
    pub fn write(&mut self, name: &str, tier: &str, body: &str) -> io::Result<()> {
        let content = fs::read_to_string(&self.path)?;
        let marker_prefix = format!("%%@{name}");

        let new_content = if let Some(start) = content.find(&marker_prefix) {
            let marker_line_end = content[start..].find('\n').map(|i| start + i + 1).unwrap_or(content.len());
            let marker_line = &content[start..marker_line_end];
            let version = parse_version(marker_line) + 1;

            let end_marker = "%%END";
            let end_pos = content[start..]
                .find(end_marker)
                .map(|i| start + i + end_marker.len())
                .unwrap_or(content.len());
            let mut rest = &content[end_pos..];
            while rest.starts_with('\n') || rest.starts_with('\r') {
                rest = &rest[1..];
            }

            format!(
                "{}%%@{} {} v:{}\n{}%%END\n{}",
                &content[..start],
                name,
                tier,
                version,
                body,
                rest
            )
        } else {
            format!("{content}\n%%@{name} {tier} v:1\n{body}%%END\n")
        };

        fs::write(&self.path, new_content)?;
        self.rebuild_index()?;
        self.reload()
    }

    /// Standing convention for "context is about to run out" — always
    /// !volatile, always safe to call repeatedly (overwrites the last one).
    pub fn checkpoint(&mut self, body: &str) -> io::Result<()> {
        self.write("checkpoint", "!volatile", body)
    }

    /// Rescan the file for "%%@name" markers and regenerate the %%INDEX
    /// block with fresh byte offsets, since edits shift everything after them.
    pub fn rebuild_index(&self) -> io::Result<()> {
        let content = fs::read_to_string(&self.path)?;

        let (header, body_start_str) = match (content.find("%%INDEX"), content.find("%%END-INDEX")) {
            (Some(i_start), Some(i_end)) => {
                let header = content[..i_start].to_string();
                let after = i_end + "%%END-INDEX".len();
                (header, content[after..].trim_start_matches(['\n', '\r']).to_string())
            }
            _ => {
                let split = content.find('\n').map(|i| i + 1).unwrap_or(0);
                (content[..split].to_string(), content[split..].to_string())
            }
        };

        let mut sections: Vec<(String, String, u32, usize)> = Vec::new();
        let mut search_from = 0usize;
        while let Some(rel) = body_start_str[search_from..].find("%%@") {
            let pos = search_from + rel;
            let line_end = body_start_str[pos..].find('\n').map(|i| pos + i).unwrap_or(body_start_str.len());
            let marker_line = &body_start_str[pos..line_end];
            if let Some((name, tier, version)) = parse_marker(marker_line) {
                sections.push((name, tier, version, pos));
            }
            search_from = pos + 3;
        }

        // Offsets are zero-padded to a FIXED width (10 digits) so the index
        // block's byte length never depends on the offset values inside it
        // -- that dependency would otherwise be circular (block length sets
        // offsets, offset digit-width changes block length). Fixed width
        // means the dummy pass and the real pass are the same length, so
        // 'base' computed from the dummy pass is exact.
        let mut index_block = String::from("%%INDEX\n");
        for (name, tier, version, _) in &sections {
            index_block.push_str(&format!("{name}:{tier}:v{version}:{:010}\n", 0));
        }
        index_block.push_str("%%END-INDEX\n");
        let base = header.len() + index_block.len();

        let mut final_index = String::from("%%INDEX\n");
        for (name, tier, version, rel_offset) in &sections {
            final_index.push_str(&format!("{name}:{tier}:v{version}:{:010}\n", base + rel_offset));
        }
        final_index.push_str("%%END-INDEX\n");

        let out = format!("{header}{final_index}{body_start_str}");
        fs::write(&self.path, out)?;
        Ok(())
    }
}

fn parse_version(marker_line: &str) -> u32 {
    marker_line
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("v:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1)
}

fn parse_marker(line: &str) -> Option<(String, String, u32)> {
    let rest = line.strip_prefix("%%@")?;
    let mut parts = rest.split_whitespace();
    let name = parts.next()?.to_string();
    let tier = parts.next().unwrap_or("!durable").to_string();
    let version = parts
        .next()
        .and_then(|v| v.strip_prefix("v:"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    Some((name, tier, version))
}

// --- quick manual smoke test: `rustc mctx.rs -o mctx_test && ./mctx_test` ---
fn main() -> io::Result<()> {
    let path = "/tmp/sample_rust.mctx";
    let mut f = File::create(path)?;
    writeln!(f, "#mctx v1.1 | updated:2026-08-08")?;
    drop(f);

    let mut store = Store::open(path)?;
    store.write("identity", "!fixed", "user{alias,role}:\n  \"devil2\",\"builder\"\n")?;
    store.checkpoint("task: verify rust port\nnext: none\n")?;

    for s in store.index() {
        println!("{} {} v{} @{}", s.name, s.tier, s.version, s.offset);
    }
    println!("\n--- checkpoint ---\n{}", store.read("checkpoint")?);
    Ok(())
}
