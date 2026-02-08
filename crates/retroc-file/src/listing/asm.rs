use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_dotted_groups, write_grouped_number,
};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use nom::combinator::all_consuming;
use nom::Parser;

/// Parse an assembly listing text into `ListingEntry`s according to `cfg`.
///
/// Expected line format: `<address> <entry> <rest...>` where address and entry
/// are dotted groups like `ffff.ffff` or `777.777` depending on `cfg.base`.
pub fn parse_asm_listing(
    text: &str,
    cfg: ListingConfig,
) -> Result<Vec<ListingEntry>, &'static str> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, char::is_whitespace);
        let addr_token = parts.next().ok_or("missing address")?;
        let entry_token = parts.next().ok_or("missing entry")?;
        let rest = parts.next().unwrap_or("");

        let (_, addr_val) = all_consuming(|i| parse_dotted_groups(i, cfg.base, cfg.addr_groups))
            .parse(addr_token)
            .map_err(|_| "invalid address")?;
        let (_, entry_val) = all_consuming(|i| parse_dotted_groups(i, cfg.base, cfg.entry_groups))
            .parse(entry_token)
            .map_err(|_| "invalid entry")?;

        let bytes = grouped_value_to_bytes(
            entry_val as u128,
            cfg.base,
            cfg.entry_group_width,
            cfg.entry_groups,
        );
        out.push(ListingEntry {
            address: addr_val as u64,
            bytes,
            text: String::from(rest),
        });
    }
    Ok(out)
}

/// Print listing entries as assembly listing lines according to `cfg` into the provided writer.
pub fn write_asm_listing<W: Write>(
    w: &mut W,
    entries: &[ListingEntry],
    cfg: ListingConfig,
) -> core::fmt::Result {
    for e in entries {
        write_grouped_number(w, e.address as u128, cfg, cfg.addr_groups, cfg.addr_group_width)?;
        w.write_char(' ')?;
        // reconstruct entry numeric from bytes (big-endian)
        let mut val: u128 = 0;
        for &b in &e.bytes {
            val = (val << 8) | (b as u128);
        }
        write_grouped_number(w, val, cfg, cfg.entry_groups, cfg.entry_group_width)?;
        if !e.text.is_empty() {
            w.write_char(' ')?;
            w.write_str(&e.text)?;
        }
        w.write_char('\n')?;
    }
    Ok(())
}

/// Print listing entries as assembly listing lines according to `cfg`.
pub fn print_asm_listing(entries: &[ListingEntry], cfg: ListingConfig) -> String {
    let mut s = String::new();
    write_asm_listing(&mut s, entries, cfg).unwrap();
    s
}
