#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use crate::listing::core::{format_grouped_number, grouped_value_to_bytes, parse_grouped_number, ListingConfig, ListingEntry};

/// Parse a "detached" listing.
///
/// In a detached listing the raw bytes are provided separately as a byte slice and
/// comment lines are provided as text. Lines that begin with an address token
/// (dotted groups) are treated as mappings: `<address> <comment>` — address is
/// matched into the provided `raw` bytes (as an offset) and the comment is kept.
///
/// This function produces `ListingEntry`s for lines that specify addresses. For
/// other lines the whole line is preserved as an entry with no bytes.
pub fn parse_detached_listing(comments: &str, raw: &[u8], cfg: ListingConfig) -> Result<Vec<ListingEntry>, &'static str> {
    let mut out = Vec::new();
    for line in comments.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        // try to split into address + rest
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let first = parts.next().unwrap();
        if first.contains('.') {
            // try parse address with expected addr_groups
            if let Ok(addr_val) = parse_grouped_number(first, cfg.base, Some(cfg.addr_groups)) {
                let rest = parts.next().unwrap_or("");
                // interpret address as offset into raw
                let offset = addr_val as usize;
                let bytes = if offset < raw.len() {
                    // take a window according to entry groups size
                    let group_bits = match cfg.base { 16 => 4 * cfg.entry_group_width, 8 => 3 * cfg.entry_group_width, _ => 4 * cfg.entry_group_width };
                    let total_bits = group_bits * cfg.entry_groups;
                    let total_bytes = (total_bits + 7) / 8;
                    let end = core::cmp::min(raw.len(), offset + total_bytes);
                    raw[offset..end].to_vec()
                } else { Vec::new() };
                out.push(ListingEntry { address: addr_val as u64, bytes, text: String::from(rest) });
                continue;
            }
        }
        // otherwise keep the line as a comment entry with no bytes and address 0
        out.push(ListingEntry { address: 0, bytes: Vec::new(), text: String::from(trimmed) });
    }
    Ok(out)
}

/// Print detached listing comments and return raw bytes file contents separately.
pub fn print_detached_listing(entries: &[ListingEntry], cfg: ListingConfig) -> (String, Vec<u8>) {
    let mut comments = String::new();
    let mut raw: Vec<u8> = Vec::new();
    for e in entries {
        if e.address != 0 {
            let addr = format_grouped_number(e.address as u128, cfg, cfg.addr_groups, cfg.addr_group_width);
            comments.push_str(&alloc::format!("{} {}\n", addr, e.text));
            // place bytes into raw at the corresponding offset; grow vector if needed
            let offset = e.address as usize;
            if raw.len() < offset { raw.resize(offset, 0); }
            raw.extend_from_slice(&e.bytes);
        } else {
            comments.push_str(&alloc::format!("{}\n", e.text));
        }
    }
    (comments, raw)
}
