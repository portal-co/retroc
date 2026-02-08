use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_grouped_number,
};
use alloc::{string::String, vec::Vec};
use nom::character::complete::{char, hex_digit1};
use nom::combinator::all_consuming;
use nom::multi::separated_list1;
use nom::{IResult, Parser};

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
            // We'll accept hex digits for base 16, or fall back to `parse_grouped_number` for octal.
            let parsed_addr = if cfg.base == 16 {
                // parse dotted hex groups
                fn parse_hex_groups(i: &str) -> IResult<&str, Vec<&str>> {
                    separated_list1(char('.'), hex_digit1).parse(i)
                }
                match all_consuming(parse_hex_groups).parse(first) {
                    Ok((_, groups)) => {
                        if groups.len() != cfg.addr_groups {
                            return Err("address group count mismatch");
                        }
                        // join groups with '.' and reuse parse_grouped_number for numeric conversion
                        let joined = groups.join(".");
                        Some(parse_grouped_number(
                            &joined,
                            cfg.base,
                            Some(cfg.addr_groups),
                        )?)
                    }
                    Err(_) => None,
                }
            } else {
                // for octal use existing helper
                Some(parse_grouped_number(
                    first,
                    cfg.base,
                    Some(cfg.addr_groups),
                )?)
            };

            if let Some(addr_val) = parsed_addr {
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

/// Print detached listing comments and return raw bytes file contents separately.
pub fn print_detached_listing(entries: &[ListingEntry], cfg: ListingConfig) -> (String, Vec<u8>) {
    let mut comments = String::new();
    let mut raw: Vec<u8> = Vec::new();
    for e in entries {
        if e.address != 0 {
            let addr = format_grouped_number(
                e.address as u128,
                cfg,
                cfg.addr_groups,
                cfg.addr_group_width,
            );
            comments.push_str(&alloc::format!("{} {}\n", addr, e.text));
            // place bytes into raw at the corresponding offset; grow vector if needed
            let offset = e.address as usize;
            if raw.len() < offset {
                raw.resize(offset, 0);
            }
            raw.extend_from_slice(&e.bytes);
        } else {
            comments.push_str(&alloc::format!("{}\n", e.text));
        }
    }
    (comments, raw)
}
