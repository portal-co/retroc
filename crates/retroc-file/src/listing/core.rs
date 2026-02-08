use alloc::{string::String, vec::Vec};
use core::fmt::{self, Display};

/// Configuration for formatting/parsing listings.
#[derive(Clone, Copy, Debug)]
pub struct ListingConfig {
    /// Numeric base: 8 or 16 are supported.
    pub base: u8,
    /// Number of groups in the address (e.g. 2 for `ffff.ffff`).
    pub addr_groups: usize,
    /// Width (digits) of each address group (e.g. 4 for hex `ffff`, 3 for octal `777`).
    pub addr_group_width: usize,
    /// Number of groups in an entry (e.g. 2 for `0000.0000`).
    pub entry_groups: usize,
    /// Width (digits) of each entry group.
    pub entry_group_width: usize,
}

impl ListingConfig {
    pub fn new_hex(
        addr_groups: usize,
        addr_group_width: usize,
        entry_groups: usize,
        entry_group_width: usize,
    ) -> Self {
        Self {
            base: 16,
            addr_groups,
            addr_group_width,
            entry_groups,
            entry_group_width,
        }
    }
    pub fn new_octal(
        addr_groups: usize,
        addr_group_width: usize,
        entry_groups: usize,
        entry_group_width: usize,
    ) -> Self {
        Self {
            base: 8,
            addr_groups,
            addr_group_width,
            entry_groups,
            entry_group_width,
        }
    }

    fn group_bits(&self) -> usize {
        // number of bits per group (base 16 => 4 bits/digit, base 8 => 3 bits/digit)
        let bits_per_digit = match self.base {
            16 => 4,
            8 => 3,
            _ => 4,
        };
        bits_per_digit * self.entry_group_width
    }
}

/// A single listing entry: address, raw bytes and text (mnemonic/comment)
#[derive(Clone, Debug)]
pub struct ListingEntry {
    pub address: u64,
    pub bytes: Vec<u8>,
    pub text: String,
}

impl Display for ListingEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}: {}", self.address, self.text)
    }
}

/// Helpers used by parsers/printers: parse a dotted group string into a numeric value.
pub fn parse_grouped_number(
    s: &str,
    base: u8,
    expected_groups: Option<usize>,
) -> Result<u128, &'static str> {
    let parts: Vec<&str> = s.split('.').collect();
    if let Some(expected) = expected_groups {
        if parts.len() != expected {
            return Err("group count mismatch");
        }
    }
    let mut value: u128 = 0;
    for part in parts {
        let part_val =
            u128::from_str_radix(part, base as u32).map_err(|_| "invalid group digits")?;
        // shift previous by sufficient bits to append next group
        let bits = match base {
            16 => 4 * part.len(),
            8 => 3 * part.len(),
            _ => 4 * part.len(),
        };
        value = (value << bits) | part_val;
    }
    Ok(value)
}

/// Format a numeric value into dotted groups with zero padding.
pub fn format_grouped_number(
    mut value: u128,
    cfg: ListingConfig,
    groups: usize,
    group_width: usize,
) -> String {
    // We'll extract groups from least-significant to most.
    let base = cfg.base as u128;
    let mut parts: Vec<String> = Vec::new();
    for _ in 0..groups {
        // mask for one group: base^width - 1
        let mask = base.pow(group_width as u32) - 1;
        let part = (value & mask) as u128;
        let s = if cfg.base == 16 {
            // hex with lowercase
            alloc::format!("{:0width$x}", part, width = group_width)
        } else {
            alloc::format!("{:0width$o}", part, width = group_width)
        };
        parts.push(s);
        value >>= match cfg.base {
            16 => 4 * group_width,
            8 => 3 * group_width,
            _ => 4 * group_width,
        };
    }
    parts.reverse();
    parts.join(".")
}

/// Convert a grouped numeric value into bytes (big-endian) where each group contributes group_bits bits.
pub fn grouped_value_to_bytes(
    mut value: u128,
    base: u8,
    group_width: usize,
    groups: usize,
) -> Vec<u8> {
    let mut out = Vec::new();
    let group_bits = match base {
        16 => 4 * group_width,
        8 => 3 * group_width,
        _ => 4 * group_width,
    };
    let mut total_bits = group_bits * groups;
    if total_bits == 0 {
        return out;
    }
    // round up to bytes
    let total_bytes = (total_bits + 7) / 8;
    // produce big-endian bytes
    for i in (0..total_bytes).rev() {
        let shift = i * 8;
        let b = ((value >> shift) & 0xff) as u8;
        out.push(b);
    }
    out
}
