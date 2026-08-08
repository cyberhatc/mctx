//! Test / demo for the `.mctx` Rust library. Builds with a single command:
//!
//!     rustc --edition 2021 -D warnings -O -o /tmp/mctx_test test_mctx.rs && /tmp/mctx_test
//!
//! Covers: fresh-file creation, appends across all three tiers, seek-read of
//! one section, in-place update with `v:` bump, checkpoint overwrite, a
//! standalone index rebuild, and — the critical check — that every stored
//! byte offset points *literally* at its `%%@name` marker in the raw file.

mod mctx;

use mctx::{Store, TIERS};
use std::fs;

fn read_expect(store: &Store, name: &str, needle: &str) -> String {
    let body = store
        .read(name)
        .unwrap_or_else(|e| panic!("read '{name}': {e}"));
    assert!(
        body.contains(needle),
        "section '{name}' missing '{needle}': {body:?}"
    );
    body
}

fn version_of(store: &Store, name: &str) -> u32 {
    store
        .index()
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("section '{name}' missing from index"))
        .version
}

/// §5 of the build spec: independently verify every offset in the index is
/// byte-exact. Read the raw file, jump to each stored offset, and confirm the
/// bytes there are literally `%%@<name>` — not a few bytes off in either
/// direction. This is the most likely silent failure of the fixed-width
/// offset design.
fn assert_byte_exact(path: &str, store: &Store) {
    let bytes = fs::read(path).expect("read raw file for byte-exact check");
    for section in store.index() {
        let marker = format!("%%@{}", section.name);
        let start = section.offset as usize;
        assert!(
            start + marker.len() <= bytes.len(),
            "offset {} for '{}' is out of file bounds",
            section.offset,
            section.name
        );
        let got = String::from_utf8_lossy(&bytes[start..start + marker.len()]);
        assert_eq!(
            got,
            marker,
            "byte-exact mismatch: section '{}' stored offset {} does not point at its %%@ marker",
            section.name,
            section.offset
        );
    }
}

fn main() {
    let path = format!("/tmp/mctx_test_{}.mctx", std::process::id());
    let _ = fs::remove_file(&path);
    fs::write(&path, "#mctx v1.1 | updated:2026-08-08\n").unwrap();

    let mut store = Store::open(&path).unwrap();

    // 1. Append sections across all three durability tiers.
    store
        .write(
            "identity",
            "!fixed",
            "user{alias,role}:\n  \"devil2\",\"student/builder\"\n",
        )
        .unwrap();
    store
        .write(
            "projects",
            "!durable",
            "projects[2]{id,title,status}:\n  p1,\"SmartTodo\",\"in progress\"\n  p2,\"Zoya\",\"active\"\n",
        )
        .unwrap();
    store
        .write(
            "log",
            "!volatile",
            "memories[1]{date,fact}:\n  \"2026-08-08\",\"built the .mctx Rust lib\"\n",
        )
        .unwrap();

    // 2. Direct seek-read of one section (never reads the rest of the file).
    read_expect(&store, "projects", "SmartTodo");
    read_expect(&store, "identity", "devil2");

    // 3. Update a section in place; version must bump 1 -> 2.
    assert_eq!(version_of(&store, "projects"), 1, "first write is v:1");
    store
        .write(
            "projects",
            "!durable",
            "projects[2]{id,title,status}:\n  p1,\"SmartTodo\",\"shipped\"\n  p2,\"Zoya\",\"active\"\n",
        )
        .unwrap();
    assert_eq!(version_of(&store, "projects"), 2, "update bumps v:");
    read_expect(&store, "projects", "shipped");

    // 4. Checkpoint: write-or-overwrite, always !volatile.
    store
        .checkpoint(
            "task: implement .mctx rust lib\nnext: wire into agent\nfiles_touched: src/mctx.rs\nblockers: none\n",
        )
        .unwrap();
    read_expect(&store, "checkpoint", "next: wire into agent");
    store
        .checkpoint("task: implement .mctx rust lib\nnext: push to github\ndone: verified byte offsets\n")
        .unwrap();
    assert_eq!(version_of(&store, "checkpoint"), 2, "checkpoint overwrites");

    // 5. Standalone rebuild (as if the file were hand-edited externally).
    store.rebuild_index().unwrap();
    store.reload().unwrap();
    assert_eq!(version_of(&store, "projects"), 2, "rebuild keeps versions");

    // 6. The critical check: every index offset must be byte-exact.
    assert_byte_exact(&path, &store);

    // 7. Tier sanity — only the three spec tiers may appear in the index.
    for section in store.index() {
        assert!(
            TIERS.contains(&section.tier.as_str()),
            "invalid tier {} on section {}",
            section.tier,
            section.name
        );
    }

    // 8. A tier outside the spec is rejected before touching the file.
    assert!(
        store.write("bad", "!eternal", "x\n").is_err(),
        "4th tier must be rejected"
    );

    // 9. Bodies without a trailing newline still read back identically.
    let path2 = format!("/tmp/mctx_test_{}_b.mctx", std::process::id());
    fs::write(&path2, "#mctx v1.1 | updated:2026-08-08\n").unwrap();
    let mut store2 = Store::open(&path2).unwrap();
    store2.write("note", "!volatile", "task: no trailing newline").unwrap();
    assert_eq!(store2.read("note").unwrap(), "task: no trailing newline\n");
    assert_byte_exact(&path2, &store2);
    let _ = fs::remove_file(&path2);

    println!("index:");
    for section in store.index() {
        println!(
            "  {:<12} {:<10} v{:<3} @{}",
            section.name, section.tier, section.version, section.offset
        );
    }
    println!(
        "\n--- projects (seek-read) ---\n{}",
        store.read("projects").unwrap()
    );
    println!(
        "\n--- checkpoint ---\n{}",
        store.read("checkpoint").unwrap()
    );
    println!("\n--- full .mctx file ---\n{}", fs::read_to_string(&path).unwrap());
    println!("\nOK — all assertions passed (byte-exact offsets verified).");

    let _ = fs::remove_file(&path);
}
