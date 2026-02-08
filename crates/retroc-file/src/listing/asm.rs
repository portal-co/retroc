use crate::listing::core::{
    ListingConfig, ListingEntry, format_grouped_number, grouped_value_to_bytes,
    parse_dotted_groups, write_grouped_number,
};
use crate::{ErrorKind, FileParser, FileRegistry};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use nom::character::complete::{char as nom_char, not_line_ending, space0, space1};
use nom::combinator::{all_consuming, map_res};
use nom::multi::many0;
use nom::sequence::{preceded, terminated, tuple};
use nom::{IResult, Parser};

/// A parser for assembly listings.
pub struct AsmParser {
    pub cfg: ListingConfig,
}

impl<Err> FileParser<Vec<ListingEntry>, Err> for AsmParser
where
    Err: From<ErrorKind> + for<'b>nom::error::ParseError<&'b str>,
{
    fn from_bytes_and_meta<'a, 'b>(
        &self,
        bytes: &'a [u8],
        mut meta: &'b str,
        _registry: &FileRegistry<'_, Vec<ListingEntry>, Err>,
    ) -> Result<(&'a [u8], &'b str, Vec<ListingEntry>), nom::Err<Err>> {
        let entries;
        (meta, entries) = parse_asm_listing(meta, self.cfg)?;
        Ok((bytes, meta, entries))
    }
}

/// A nom parser for a single line of an assembly listing.
fn parse_asm_line<'a, E: nom::error::ParseError<&'a str>>(
    i: &'a str,
    cfg: ListingConfig,
) -> IResult<&'a str, ListingEntry, E> {
    let mut addr_p = |i| parse_dotted_groups(i, cfg.base, cfg.addr_groups);
    let mut entry_p = |i| parse_dotted_groups(i, cfg.base, cfg.entry_groups);

    let (i, (_, addr_val, _, entry_val, rest)) = (
        space0,
        addr_p,
        space1,
        entry_p,
        preceded(space0, not_line_ending),
    )
        .parse(i)?;

    let bytes =
        grouped_value_to_bytes(entry_val, cfg.base, cfg.entry_group_width, cfg.entry_groups);

    Ok((
        i,
        ListingEntry {
            address: addr_val as u64,
            bytes,
            text: String::from(rest),
        },
    ))
}

/// Parse an assembly listing text into `ListingEntry`s according to `cfg`.
pub fn parse_asm_listing<'a, E: nom::error::ParseError<&'a str>>(
    text: &'a str,
    cfg: ListingConfig,
) -> IResult<&'a str, Vec<ListingEntry>, E> {
    many0(terminated(|i| parse_asm_line(i, cfg), nom_char('\n'))).parse(text)
}

/// Print listing entries as assembly listing lines according to `cfg` into the provided writer.
pub fn write_asm_listing<W: Write>(
    w: &mut W,
    entries: &[ListingEntry],
    cfg: ListingConfig,
) -> core::fmt::Result {
    for e in entries {
        write_grouped_number(
            w,
            e.address as u128,
            cfg,
            cfg.addr_groups,
            cfg.addr_group_width,
        )?;
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
