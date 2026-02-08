use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_grouped_number,
};
use alloc::{string::String, vec::Vec};
use nom::Parser;
use nom::bytes::take_while1;
use nom::character::complete::hex_digit1;

use core::fmt::Write;

use nom::combinator::all_consuming;
use nom::multi::separated_list1;

fn parse_dotted_groups(token: &str, base: u8, expected: usize) -> Result<u128, &'static str> {
    if base == 16 {
        fn parse_hex(i: &str) -> nom::IResult<&str, Vec<&str>> {
            separated_list1(nom::character::complete::char('.'), hex_digit1).parse(i)
        }
        match all_consuming(parse_hex).parse(token) {
            Ok((_, groups)) => {
                if groups.len() != expected {
                    return Err("group count mismatch");
                }
                let joined = groups.join(".");
                parse_grouped_number(&joined, base, Some(expected))
            }
            Err(_) => Err("invalid hex groups"),
        }
    } else if base == 8 {
        fn octal_group(i: &str) -> nom::IResult<&str, &str> {
            take_while1(|c: char| c >= '0' && c <= '7').parse(i)
        }
        fn parse_octal(i: &str) -> nom::IResult<&str, Vec<&str>> {
            separated_list1(nom::character::complete::char('.'), octal_group).parse(i)
        }
        match all_consuming(parse_octal).parse(token) {
            Ok((_, groups)) => {
                if groups.len() != expected {
                    return Err("group count mismatch");
                }
                let joined = groups.join(".");
                parse_grouped_number(&joined, base, Some(expected))
            }
            Err(_) => Err("invalid octal groups"),
        }
    } else {
        Err("unsupported base")
    }
}

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

        let addr_val = parse_dotted_groups(addr_token, cfg.base, cfg.addr_groups)?;
        let entry_val = parse_dotted_groups(entry_token, cfg.base, cfg.entry_groups)?;
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

/// Print listing entries as assembly listing lines according to `cfg`.
pub fn print_asm_listing(entries: &[ListingEntry], cfg: ListingConfig) -> String {
    let mut s = String::new();
    for e in entries {
        let addr = format_grouped_number(
            e.address as u128,
            cfg,
            cfg.addr_groups,
            cfg.addr_group_width,
        );
        // reconstruct entry numeric from bytes (big-endian)
        let mut val: u128 = 0;
        for &b in &e.bytes {
            val = (val << 8) | (b as u128);
        }
        let entry = format_grouped_number(val, cfg, cfg.entry_groups, cfg.entry_group_width);
        if e.text.is_empty() {
            write!(s, "{} {}\n", addr, entry).unwrap();
        } else {
            write!(s, "{} {} {}\n", addr, entry, e.text).unwrap();
        }
    }
    s
}
