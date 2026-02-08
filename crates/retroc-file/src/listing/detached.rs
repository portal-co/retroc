use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_dotted_groups, write_grouped_number,
};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use nom::combinator::all_consuming;
use nom::Parser;

/// Parse a "detached" listing.
///
/// In a detached listing the raw bytes are provided separately as a byte slice and
/// comment lines are provided as text. Lines that begin with an address token
/// (dotted groups) are treated as mappings: `<address> <comment>` — address is
/// matched into the provided `raw` bytes (as an offset) and the comment is kept.
///
/// This function produces `ListingEntry`s for lines that specify addresses. For
/// other lines the whole line is preserved as an entry with no bytes.
pub fn parse_detached_listing(
    comments: &str,
    raw: &[u8],
    cfg: ListingConfig,
) -> Result<Vec<ListingEntry>, &'static str> {
    let mut out = Vec::new();
    let mut cursor: usize = 0;
    for line in comments.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // split into first token and rest
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let first = parts.next().unwrap();
        let rest = parts.next().unwrap_or("");

        // If token looks like dotted groups, handle as potential address
        if first.contains('.') {
            // quick check for all-underscore token: e.g. "__.__" or "____"
            let is_all_underscores = first.chars().all(|c| c == '_' || c == '.');
            if is_all_underscores {
                // infer address from cursor
                let addr_val = cursor as u128;
                // compute bytes window according to cfg
                let group_bits = match cfg.base {
                    16 => 4 * cfg.entry_group_width,
                    8 => 3 * cfg.entry_group_width,
                    _ => 4 * cfg.entry_group_width,
                };
                let total_bits = group_bits * cfg.entry_groups;
                let total_bytes = (total_bits + 7) / 8;
                let offset = addr_val as usize;
                let end = core::cmp::min(raw.len(), offset + total_bytes);
                let bytes = if offset < raw.len() {
                    raw[offset..end].to_vec()
                } else {
                    Vec::new()
                };
                out.push(ListingEntry {
                    address: addr_val as u64,
                    bytes: bytes.clone(),
                    text: String::from(rest),
                });
                cursor = (addr_val as usize).saturating_add(bytes.len());
                continue;
            }

            // Otherwise parse numeric address using `nom` for robustness.
            if let Ok((_, addr_val)) = all_consuming(|i| parse_dotted_groups(i, cfg.base, cfg.addr_groups)).parse(first) {
                let offset = addr_val as usize;
                let group_bits = match cfg.base {
                    16 => 4 * cfg.entry_group_width,
                    8 => 3 * cfg.entry_group_width,
                    _ => 4 * cfg.entry_group_width,
                };
                let total_bits = group_bits * cfg.entry_groups;
                let total_bytes = (total_bits + 7) / 8;
                let end = core::cmp::min(raw.len(), offset + total_bytes);
                let bytes = if offset < raw.len() {
                    raw[offset..end].to_vec()
                } else {
                    Vec::new()
                };
                out.push(ListingEntry {
                    address: addr_val as u64,
                    bytes: bytes.clone(),
                    text: String::from(rest),
                });
                cursor = offset.saturating_add(bytes.len());
                continue;
            }
        }

        // otherwise treat as a comment-only line
        out.push(ListingEntry {
            address: 0,
            bytes: Vec::new(),
            text: String::from(trimmed),
        });
    }
    Ok(out)
}

/// Print detached listing comments into a writer and return raw bytes separately.
pub fn write_detached_listing<W: Write>(
    w: &mut W,
    entries: &[ListingEntry],
    cfg: ListingConfig,
) -> (core::fmt::Result, Vec<u8>) {
    let mut raw: Vec<u8> = Vec::new();
    let mut res = Ok(());
    for e in entries {
        if e.address != 0 {
            if let Err(e) = write_grouped_number(w, e.address as u128, cfg, cfg.addr_groups, cfg.addr_group_width) {
                res = Err(e);
            }
            if let Err(e) = w.write_char(' ') {
                res = Err(e);
            }
            if let Err(e) = w.write_str(&e.text) {
                res = Err(e);
            }
            if let Err(e) = w.write_char('\n') {
                res = Err(e);
            }
            // place bytes into raw at the corresponding offset; grow vector if needed
            let offset = e.address as usize;
            if raw.len() < offset {
                raw.resize(offset, 0);
            }
            raw.extend_from_slice(&e.bytes);
        } else {
            if let Err(e) = w.write_str(&e.text) {
                res = Err(e);
            }
            if let Err(e) = w.write_char('\n') {
                res = Err(e);
            }
        }
    }
    (res, raw)
}

/// Print detached listing comments and return raw bytes file contents separately.
pub fn print_detached_listing(entries: &[ListingEntry], cfg: ListingConfig) -> (String, Vec<u8>) {
    let mut comments = String::new();
    let (_, raw) = write_detached_listing(&mut comments, entries, cfg);
    (comments, raw)
}
