//! `.mctx` — Memory Context Format: a token-optimized, seek-indexed memory
//! file for AI agents. Lightweight Rust reader/writer. No external crates.
//!
//! File layout (v1.1):
//! ```text
//! #mctx v1.1 | updated:<ISO date>
//! %%INDEX
//! <name>:<tier>:v<N>:<byte-offset>     // offsets zero-padded to 10 digits
//! %%END-INDEX
//! %%@<name> <tier> v:<N>
//! <body>                               // tabular arrays or `key: value` lines
//! %%END
//! ```
//!
//! Reading a section is index-first, then one `seek` straight to its byte
//! offset — the body of every other section is never read. Writing edits one
//! section, bumps its `v:`, then rewrites the whole file and rebuilds the
//! index, since any edit shifts every later byte offset.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};

/// Tier tags allowed by the spec — there is deliberately no fourth tier.
pub const TIERS: [&str; 3] = ["!fixed", "!durable", "!volatile"];

/// Every byte offset in the index is zero-padded to this width so the index
/// block's byte length never depends on the offset values it contains.
/// Otherwise block length and offsets would be circularly defined: the block
/// length fixes where sections start, but each offset's digit-width changes
/// the block length. A fixed width makes a dummy pass and the real pass
/// identical in length, so `base` computed from the dummy pass is exact.
pub const OFFSET_WIDTH: usize = 10;

/// One row of the `%%INDEX` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub tier: String,
    pub version: u32,
    /// Exact byte position of the section's `%%@name` marker line.
    pub offset: u64,
}

/// A handle to one `.mctx` file. Cheap to keep around: it only ever holds the
/// loaded index, never section bodies.
pub struct Store {
    path: String,
    index: Vec<Section>,
}

impl Store {
    pub fn open(path: &str) -> io::Result<Self> {
        let mut store = Store {
            path: path.to_string(),
            index: Vec::new(),
        };
        store.reload()?;
        Ok(store)
    }

    /// The currently loaded index — section name, tier, version, byte offset.
    pub fn index(&self) -> &[Section] {
        &self.index
    }

    /// Parse ONLY the `%%INDEX ... %%END-INDEX` block at the top of the file.
    /// Stops reading at `%%END-INDEX`, so section bodies are never touched.
    pub fn reload(&mut self) -> io::Result<()> {
        let reader = BufReader::new(File::open(&self.path)?);
        self.index.clear();

        let mut in_index = false;
        for line in reader.lines() {
            let line = line?;
            if line.starts_with("%%INDEX") {
                in_index = true;
                continue;
            }
            if line.starts_with("%%END-INDEX") {
                break;
            }
            if !in_index || line.trim().is_empty() {
                continue;
            }
            if let Some(section) = parse_index_line(&line) {
                self.index.push(section);
            }
        }
        Ok(())
    }

    /// Seek straight to the section's byte offset and read until `%%END` —
    /// never reads the rest of the file. Also doubles as a runtime check that
    /// the stored offset points exactly at the `%%@name` marker.
    pub fn read(&self, name: &str) -> io::Result<String> {
        let section = self
            .index
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| not_found(name))?;

        let mut reader = BufReader::new(File::open(&self.path)?);
        reader.seek(SeekFrom::Start(section.offset))?;

        let mut marker = String::new();
        reader.read_line(&mut marker)?;
        if !marker.starts_with("%%@") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "stored offset {} for '{name}' does not point at a %%@ marker",
                    section.offset
                ),
            ));
        }

        let mut body = String::new();
        for line in reader.lines() {
            let line = line?;
            if line.starts_with("%%END") {
                break;
            }
            body.push_str(&line);
            body.push('\n');
        }
        Ok(body)
    }

    /// Update a section in place (bumping its `v:`) or append it as a new
    /// section (`v:1`), then rebuild the index. `tier` must be one of the
    /// three spec tiers.
    pub fn write(&mut self, name: &str, tier: &str, body: &str) -> io::Result<()> {
        if !TIERS.contains(&tier) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid tier '{tier}' (must be one of {TIERS:?})"),
            ));
        }

        let mut content = fs::read_to_string(&self.path)?;
        let marker = format!("%%@{name}");

        if let Some(start) = content.find(&marker) {
            let end = section_end(&content, start);
            let version = parse_marker_version(&content[start..]) + 1;
            content.replace_range(start..end, &section_block(name, tier, version, body));
        } else {
            content.push_str(&format!("\n{}", section_block(name, tier, 1, body)));
        }

        fs::write(&self.path, &content)?;
        self.rebuild_index()?;
        self.reload()
    }

    /// Convenience for the "about to run out of context, save state now"
    /// pattern — always `!volatile`, overwrites the previous checkpoint.
    pub fn checkpoint(&mut self, body: &str) -> io::Result<()> {
        self.write("checkpoint", "!volatile", body)
    }

    /// Rescan the whole file for `%%@name` markers and regenerate the
    /// `%%INDEX` block with fresh fixed-width byte offsets. Call standalone
    /// after section bodies were hand-edited by another tool.
    pub fn rebuild_index(&self) -> io::Result<()> {
        let content = fs::read_to_string(&self.path)?;

        // Header = everything before the %%INDEX block. If there is no index
        // yet (fresh file), the first line is the header.
        let (header, body) = match (
            content.find("%%INDEX"),
            content.find("%%END-INDEX"),
        ) {
            (Some(i_start), Some(i_end)) => {
                let header = content[..i_start].to_string();
                let after = i_end + "%%END-INDEX".len();
                let body = content[after..]
                    .trim_start_matches(['\n', '\r'])
                    .to_string();
                (header, body)
            }
            _ => {
                let split = content
                    .find('\n')
                    .map(|i| i + 1)
                    .unwrap_or(content.len());
                (content[..split].to_string(), content[split..].to_string())
            }
        };

        // Scan the body for markers, recording byte offsets relative to the
        // body start. The final absolute offset = header.len() + index block
        // length + this relative offset.
        let mut sections: Vec<(String, String, u32, usize)> = Vec::new();
        let mut from = 0usize;
        while let Some(rel) = body[from..].find("%%@") {
            let pos = from + rel;
            let line_end = body[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(body.len());
            if let Some((name, tier, version)) = parse_marker(&body[pos..line_end]) {
                sections.push((name, tier, version, pos));
            }
            from = pos + 3;
        }

        // Dummy pass (offsets = 0) fixes the index block's length; because
        // offsets are fixed-width, the real pass has the identical length.
        let dummy = index_block(&sections, 0);
        let base = header.len() + dummy.len();
        let final_index = index_block(&sections, base);

        fs::write(&self.path, format!("{header}{final_index}{body}"))?;
        Ok(())
    }
}

/// Build the `%%INDEX` block for the given sections. `base` is the byte
/// position where the section bodies begin; the final offset is `base + rel`.
fn index_block(sections: &[(String, String, u32, usize)], base: usize) -> String {
    let mut block = String::from("%%INDEX\n");
    for (name, tier, version, rel) in sections {
        let offset = base + rel;
        block.push_str(&format!(
            "{name}:{tier}:v{version}:{offset:0width$}\n",
            width = OFFSET_WIDTH
        ));
    }
    block.push_str("%%END-INDEX\n");
    block
}

/// Render one `%%@name tier v:N ... %%END` block, guaranteeing a newline
/// before `%%END` even when the body has no trailing newline.
fn section_block(name: &str, tier: &str, version: u32, body: &str) -> String {
    if body.ends_with('\n') {
        format!("%%@{name} {tier} v:{version}\n{body}%%END\n")
    } else {
        format!("%%@{name} {tier} v:{version}\n{body}\n%%END\n")
    }
}

/// Index of the byte just past a section's `%%END` terminator line.
fn section_end(content: &str, from: usize) -> usize {
    let rel = content[from..]
        .find("%%END")
        .unwrap_or(content.len() - from);
    let mut end = from + rel + "%%END".len();
    for byte in content[end..].bytes() {
        if byte == b'\n' || byte == b'\r' {
            end += 1;
        } else {
            break;
        }
    }
    end
}

/// Parse one `%%INDEX` line: `name:tier:vN:<offset>`.
fn parse_index_line(line: &str) -> Option<Section> {
    let mut parts = line.splitn(4, ':');
    let name = parts.next()?;
    let tier = parts.next()?;
    let version = parts.next()?.strip_prefix('v')?.parse().unwrap_or(1);
    let offset = parts.next()?.parse().ok()?;
    Some(Section {
        name: name.to_string(),
        tier: tier.to_string(),
        version,
        offset,
    })
}

/// Parse a section marker line: `%%@name <tier> v:<N>`.
fn parse_marker(line: &str) -> Option<(String, String, u32)> {
    let rest = line.strip_prefix("%%@")?;
    let mut parts = rest.split_whitespace();
    let name = parts.next()?.to_string();
    let tier = parts.next().unwrap_or("!durable").to_string();
    let version = parts
        .next()
        .and_then(|tok| tok.strip_prefix("v:"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    Some((name, tier, version))
}

/// Pull just the `v:N` version out of a marker line. Scoped to the first
/// line only, so body tokens that happen to look like `v:...` can't confuse it.
fn parse_marker_version(line: &str) -> u32 {
    let first_line = line.split('\n').next().unwrap_or(line);
    first_line
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("v:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1)
}

fn not_found(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("section not in index: {name}"),
    )
}
