use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_dotted_groups, write_grouped_number,
};
use crate::{ByteMetaParser, ErrorKind};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use nom::bytes::take_while1;
use nom::character::complete::{char as nom_char, not_line_ending, space0, space1};
use nom::combinator::{all_consuming, map, opt};
use nom::multi::many0;
use nom::sequence::{preceded, terminated, tuple};
use nom::{IResult, Parser};

/// A parser for detached listings.
pub struct DetachedParser {
    pub cfg: ListingConfig,
}

impl<Err> ByteMetaParser<Vec<ListingEntry>, Err> for DetachedParser
where
    Err: From<ErrorKind>
        + for<'a> nom::error::ParseError<&'a str>
        + for<'b> nom::error::ParseError<&'b [u8]>,
{
    fn from_bytes_and_meta<'a, 'b>(
        &self,
        bytes: &'a [u8],
        meta: &'b str,
    ) -> Result<(&'a [u8], &'b str, Vec<ListingEntry>), nom::Err<Err>> {
        Ok(parse_detached_listing(bytes, meta, self.cfg)?)
    }
}

/// A nom parser for a potential address token, supporting underscores for inference.
fn parse_detached_addr<'a, E: nom::error::ParseError<&'a str>>(
    i: &'a str,
    cfg: ListingConfig,
) -> IResult<&'a str, Option<u128>, E> {
    // Try to parse underscore address first
    if let Ok((i, _)) = take_while1::<_, _, E>(|c| c == '_' || c == '.').parse(i) {
        return Ok((i, None));
    }
    // Otherwise try explicit dotted groups
    let (i, val) = parse_dotted_groups(i, cfg.base, cfg.addr_groups)?;
    Ok((i, Some(val)))
}

/// Parse a "detached" listing.
pub fn parse_detached_listing<
    'a,
    'b,
    E: nom::error::ParseError<&'b str> + nom::error::ParseError<&'a [u8]>,
>(
    mut raw: &'a [u8],
    meta: &'b str,
    cfg: ListingConfig,
) -> Result<(&'a [u8], &'b str, Vec<ListingEntry>), nom::Err<E>> {
    let mut out = Vec::new();
    let mut cursor: usize = 0;
    let mut current_input = meta;

    while !current_input.is_empty() {
        // Simple manual line split to maintain state easily while using nom for line content
        let (line, rest_input) = match current_input.split_once('\n') {
            Some((l, r)) => (l, r),
            None => (current_input, ""),
        };

        if !line.trim().is_empty() {
            let row_res: IResult<&str, (Option<Option<u128>>, &str), E> = (
                preceded(space0, opt(|i| parse_detached_addr(i, cfg))),
                preceded(space0, not_line_ending),
            )
                .parse(line);

            if let Ok((_, (addr_opt, text))) = row_res {
                if let Some(addr_val_opt) = addr_opt {
                    let addr_val = addr_val_opt.unwrap_or(cursor as u128);
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
                        text: String::from(text),
                    });
                    cursor = offset.saturating_add(bytes.len());
                } else {
                    // No address detected, just text
                    out.push(ListingEntry {
                        address: 0,
                        bytes: Vec::new(),
                        text: String::from(line.trim()),
                    });
                }
            }
        }
        current_input = rest_input;
    }

    Ok((&raw[cursor..], "", out))
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
            if let Err(e) = write_grouped_number(
                w,
                e.address as u128,
                cfg,
                cfg.addr_groups,
                cfg.addr_group_width,
            ) {
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
