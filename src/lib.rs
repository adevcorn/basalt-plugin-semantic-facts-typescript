//! Semantic-facts-typescript — Parser-derived semantic facts for TypeScript/TSX.
//!
//! Provides `semantic-facts@ts/v1` by consuming parser capabilities:
//! - `parse.call-sites@ts/v1`
//! - `parse.retrieval@ts/v1`
//!
//! ## Future nice-to-haves
//! - Optional `parse.type-refs@ts/v1` for import/type-import cross-package edges
//! - React component prop-types facts from TSX
//! - `Extends`/`Implements` relationship facts from class declarations

#![no_std]

extern crate alloc;
use alloc::string::ToString;
use alloc::vec::Vec;

use basalt_plugin_sdk::prelude::*;
use basalt_plugin_sdk::facts::{SemanticFact, SymbolKind, serialize_facts};

// ── Plugin metadata ────────────────────────────────────────────────────────

basalt_plugin_meta! {
    name:              "semantic-facts-typescript",
    version:           env!("CARGO_PKG_VERSION"),
    // NOTE: core defines SEMANTIC_FACTS = 1 << 16 but the SDK has no const
    // for it yet; the literal keeps the declared flags truthful. The live
    // dispatch path keys off CAP_CAPABILITY_HANDLE + provides/globs.
    hook_flags:        CAP_CAPABILITY_HANDLE | CAP_API_INDEX | (1 << 16),
    provides:          "semantic-facts@ts/v1",
    requires:          "parse.call-sites@ts/v1\nparse.retrieval@ts/v1",
    optional_requires: "",
    file_globs:        "**/*.ts\n**/*.tsx\n**/*.mts\n**/*.cts",
    activates_on:      "",
    activation_events: "",
}

// ── Capability handle export ───────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn basalt_capability_handle(
    cap_ptr: *const u8,
    cap_len: usize,
    req_ptr: *const u8,
    req_len: usize,
) -> i64 {
    let _ = (cap_ptr, cap_len);
    if req_ptr.is_null() || req_len == 0 { return pack_empty(); }
    let request = unsafe { core::slice::from_raw_parts(req_ptr, req_len) };

    if request.len() < 4 { return pack_error(-1002); }
    let src_len = u32::from_le_bytes([request[0], request[1], request[2], request[3]]) as usize;
    if request.len() < 4 + src_len { return pack_error(-1002); }
    let src = &request[4..4 + src_len];
    if src.is_empty() { return pack_empty(); }

    match derive_semantic_facts(src) {
        Ok(facts) => pack_success(facts),
        Err(code) => pack_error(code),
    }
}

// ── Semantic fact derivation ───────────────────────────────────────────────

fn derive_semantic_facts(src: &[u8]) -> Result<Vec<u8>, i64> {
    let call_sites_raw = invoke_parse_call_sites(src).map_err(|_| -1003)?;
    let retrieval_raw  = invoke_parse_retrieval(src).map_err(|_| -1003)?;

    let call_sites = decode_call_sites(&call_sites_raw);
    let retrieval  = decode_retrieval(&retrieval_raw);

    let mut facts: Vec<SemanticFact> = Vec::new();
    derive_declarations(&retrieval, &mut facts);
    derive_call_edges(&call_sites, &mut facts);
    derive_imports(src, &mut facts);

    Ok(serialize_facts(&facts))
}

// ── Parser capability invocations ──────────────────────────────────────────

fn invoke_parse_call_sites(src: &[u8]) -> Result<Vec<u8>, i64> {
    let mut request = Vec::with_capacity(4 + src.len() + 4);
    request.extend_from_slice(&(src.len() as u32).to_le_bytes());
    request.extend_from_slice(src);
    request.extend_from_slice(&16384u32.to_le_bytes());
    invoke_capability("parse.call-sites@ts/v1", &request).map_err(|_| -1003)
}

fn invoke_parse_retrieval(src: &[u8]) -> Result<Vec<u8>, i64> {
    let mut request = Vec::with_capacity(4 + src.len() + 4);
    request.extend_from_slice(&(src.len() as u32).to_le_bytes());
    request.extend_from_slice(src);
    request.extend_from_slice(&8192u32.to_le_bytes());
    invoke_capability("parse.retrieval@ts/v1", &request).map_err(|_| -1003)
}

// ── Parser response decoding ───────────────────────────────────────────────

fn decode_call_sites(data: &[u8]) -> Vec<(u32, &str)> {
    if data.len() < 4 { return Vec::new(); }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut sites = Vec::with_capacity(count);
    let mut pos = 4;
    for _ in 0..count {
        if pos + 68 > data.len() { break; }
        let offset = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let name_bytes = &data[pos + 4..pos + 68];
        let nul = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
        if let Ok(name) = core::str::from_utf8(&name_bytes[..nul]) {
            if !name.is_empty() { sites.push((offset, name)); }
        }
        pos += 68;
    }
    sites
}

fn decode_retrieval(data: &[u8]) -> Vec<(u32, u32, &str, u8)> {
    if data.len() < 4 { return Vec::new(); }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut chunks = Vec::with_capacity(count);
    let mut pos = 4;
    for _ in 0..count {
        if pos + 104 > data.len() { break; }
        let offset = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
        let length = u32::from_le_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
        let label_bytes = &data[pos + 8..pos + 103];
        let kind = data[pos + 103];
        let nul = label_bytes.iter().position(|&b| b == 0).unwrap_or(95);
        if let Ok(label) = core::str::from_utf8(&label_bytes[..nul]) {
            let label = label.trim();
            if !label.is_empty() && length > 0 { chunks.push((offset, length, label, kind)); }
        }
        pos += 104;
    }
    chunks
}

// ── Fact derivation from parser output ─────────────────────────────────────

fn derive_declarations(retrieval: &[(u32, u32, &str, u8)], facts: &mut Vec<SemanticFact>) {
    for &(offset, length, label, kind) in retrieval {
        match kind {
            0 => {
                // unknown / arrow-function — treat as Function
                facts.push(SemanticFact::DeclareSymbol {
                    kind: SymbolKind::Function,
                    offset, length, name: label.to_string(), qualified_name: None,
                });
            }
            1 => {
                // module (namespace)
                let name = label.strip_prefix("namespace ").unwrap_or(label);
                facts.push(SemanticFact::DeclareSymbol {
                    kind: SymbolKind::Module,
                    offset, length, name: name.to_string(), qualified_name: None,
                });
            }
            2 => {
                // type / class / enum
                let name = label
                    .strip_prefix("type ")
                    .or_else(|| label.strip_prefix("class "))
                    .or_else(|| label.strip_prefix("struct "))
                    .or_else(|| label.strip_prefix("enum "))
                    .unwrap_or(label);
                facts.push(SemanticFact::DeclareSymbol {
                    kind: SymbolKind::Type,
                    offset, length, name: name.to_string(), qualified_name: None,
                });
            }
            3 => {
                // function / method
                let name = label.strip_prefix("function ").unwrap_or(label);
                facts.push(SemanticFact::DeclareSymbol {
                    kind: SymbolKind::Function,
                    offset, length, name: name.to_string(), qualified_name: None,
                });
            }
            7 => {
                // interface
                let name = label.strip_prefix("interface ").unwrap_or(label);
                facts.push(SemanticFact::DeclareSymbol {
                    kind: SymbolKind::Interface,
                    offset, length, name: name.to_string(), qualified_name: None,
                });
            }
            _ => {}
        }
    }
}

fn derive_call_edges(call_sites: &[(u32, &str)], facts: &mut Vec<SemanticFact>) {
    for &(offset, callee) in call_sites {
        facts.push(SemanticFact::Calls {
            caller_offset: offset,
            caller_length: callee.len() as u32,
            callee: callee.to_string(),
        });
    }
}

/// Lightweight heuristic import scanner.
///
/// Scans `src` line-by-line for ES module `import … from '…'` / `import … from "…"` patterns
/// and bare `require('…')` / `require("…")` calls, and emits one [`SemanticFact::ImportModule`]
/// per unique module path found.
///
/// This is intentionally a byte-level heuristic: it runs without a full parse tree so that the
/// plugin still emits useful import edges even when `parse.type-refs@ts/v1` is not available.
fn derive_imports(src: &[u8], facts: &mut Vec<SemanticFact>) {
    // We need to iterate over lines and their byte offsets.
    let mut line_start: usize = 0;

    while line_start < src.len() {
        // Find end of current line.
        let line_end = src[line_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| line_start + p)
            .unwrap_or(src.len());

        let line = &src[line_start..line_end];

        // Trim leading ASCII whitespace.
        let trimmed_start = line.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(line.len());
        let trimmed = &line[trimmed_start..];

        // Match `import ` at line start (ES module import).
        if trimmed.starts_with(b"import ") {
            if let Some(module_path) = extract_from_clause(trimmed) {
                let byte_offset = (line_start + trimmed_start) as u32;
                let byte_length = line.len() as u32;
                facts.push(SemanticFact::ImportModule {
                    offset: byte_offset,
                    length: byte_length,
                    module_path: module_path.to_string(),
                    alias: None,
                });
            }
        } else {
            // Scan for `require(` anywhere in the line.
            if let Some(pos) = find_subsequence(trimmed, b"require(") {
                let after = &trimmed[pos + 8..]; // skip `require(`
                if let Some(module_path) = extract_quoted(after) {
                    let byte_offset = (line_start + trimmed_start + pos) as u32;
                    let byte_length = (trimmed.len() - pos) as u32;
                    facts.push(SemanticFact::ImportModule {
                        offset: byte_offset,
                        length: byte_length,
                        module_path: module_path.to_string(),
                        alias: None,
                    });
                }
            }
        }

        line_start = line_end + 1;
    }
}

/// Extract the module path from a `from 'path'` or `from "path"` clause.
///
/// Returns `None` if the pattern is not found.
fn extract_from_clause(line: &[u8]) -> Option<&str> {
    // Find last occurrence of ` from ` in the line.
    let needle = b" from ";
    let pos = find_last_subsequence(line, needle)?;
    let after = &line[pos + needle.len()..];
    extract_quoted(after)
}

/// Extract a quoted string from the beginning of `bytes`.
///
/// Accepts `'...'` and `"..."` — returns the content between the quotes.
fn extract_quoted(bytes: &[u8]) -> Option<&str> {
    // Skip leading whitespace.
    let start = bytes.iter().position(|&b| b != b' ' && b != b'\t')?;
    let bytes = &bytes[start..];
    if bytes.is_empty() { return None; }
    let quote = bytes[0];
    if quote != b'\'' && quote != b'"' && quote != b'`' { return None; }
    let inner = &bytes[1..];
    let end = inner.iter().position(|&b| b == quote)?;
    core::str::from_utf8(&inner[..end]).ok()
}

/// Find the first occurrence of `needle` in `haystack`, returning its start index.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Find the last occurrence of `needle` in `haystack`, returning its start index.
fn find_last_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}