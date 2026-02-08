#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use crate::listing::core::{format_grouped_number, grouped_value_to_bytes, parse_grouped_number, ListingConfig, ListingEntry};

/// Parse an assembly listing text into `ListingEntry`s according to `cfg`.
///
/// Expected line format: `<address> <entry> <rest...>` where address and entry
/// are dotted groups like `ffff.ffff` or `777.777` depending on `cfg.base`.
pub fn parse_asm_listing(text: &str, cfg: ListingConfig) -> Result<Vec<ListingEntry>, &'static str> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(3, char::is_whitespace);
        let addr_token = parts.next().ok_or("missing address")?;
        let entry_token = parts.next().ok_or("missing entry")?;
        let rest = parts.next().unwrap_or("");
        let addr_val = parse_grouped_number(addr_token, cfg.base, Some(cfg.addr_groups))?;
        let entry_val = parse_grouped_number(entry_token, cfg.base, Some(cfg.entry_groups))?;
        let bytes = grouped_value_to_bytes(entry_val as u128, cfg.base, cfg.entry_group_width, cfg.entry_groups);
        out.push(ListingEntry { address: addr_val as u64, bytes, text: String::from(rest) });
    }
    Ok(out)
}

/// Print listing entries as assembly listing lines according to `cfg`.
pub fn print_asm_listing(entries: &[ListingEntry], cfg: ListingConfig) -> String {
    let mut s = String::new();
    for e in entries {
        let addr = format_grouped_number(e.address as u128, cfg, cfg.addr_groups, cfg.addr_group_width);
        // reconstruct entry numeric from bytes (big-endian)
        let mut val: u128 = 0;
        for &b in &e.bytes {
            val = (val << 8) | (b as u128);
        }
        let entry = format_grouped_number(val, cfg, cfg.entry_groups, cfg.entry_group_width);
        if e.text.is_empty() {
            s.push_str(&alloc::format!("{} {}\n", addr, entry));
        } else {
            s.push_str(&alloc::format!("{} {} {}\n", addr, entry, e.text));
        }
    }
    s
}
