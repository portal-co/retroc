//! Structured annotation layer for listing entry text fields.
//!
//! # Text field grammar
//!
//! Every [`ListingEntry::text`] field is treated as an optional **mnemonic**
//! followed by an optional **comment**, separated by a semicolon:
//!
//! ```text
//! line-text = [ mnemonic ] [ ";" comment ]
//! ```
//!
//! Whitespace immediately surrounding the `;` separator, the mnemonic, and
//! the comment prose is trimmed during parsing but preserved faithfully when
//! the value is re-serialised through [`LineText::to_text`].
//!
//! ## Mnemonic
//!
//! Anything before the first `;` on the line (trimmed).  May be absent — a
//! line that starts with `;` is a **pure-annotation** line and carries no
//! instruction text.
//!
//! ## Comment and tags
//!
//! The comment is the text after the `;`.  Within the comment, **tags** may
//! appear anywhere, using the syntax:
//!
//! ```text
//! tag = "@[" tag-content "]"
//! ```
//!
//! A tag's content is the raw bytes between `@[` and the first `]`.  Tags
//! may not be nested.  The `@[` introducer and the `]` terminator are
//! consumed and do not appear in [`Tag::content`].
//!
//! ### Tag content conventions
//!
//! Tag content is free-form text, but by convention tools use a
//! `key value…` layout:
//!
//! | Example | Meaning |
//! |---|---|
//! | `@[label entry]` | symbolic label `entry` at this address |
//! | `@[type u8]` | annotated data type |
//! | `@[ref ffff]` | cross-reference to address `ffff` |
//! | `@[note …]` | free-form note that tools may index |
//! | `@[ibt ff00 ff20]` | possible indirect branch targets (hex addrs, space-sep) |
//! | `@[smc ff10 imm]` | operand field `imm` is patched at runtime by SMC at `ff10` |
//!
//! Tags with no recognised key are preserved transparently so that
//! forward-compatible files round-trip without data loss.
//!
//! ## Structured tag interpretation
//!
//! Call [`Tag::parse_known`] to interpret a raw [`Tag`] as a [`KnownTag`]
//! variant.  [`KnownTag::to_tag`] converts back to a raw [`Tag`] for
//! serialisation.  Use [`LineText::indirect_branch_targets`] and
//! [`LineText::smc_patches`] for direct access to the two new structured
//! kinds.
//!
//! ### Indirect branch targets (`ibt`)
//!
//! ```text
//! @[ibt addr …]
//! ```
//!
//! Attached to an indirect branch instruction (e.g. `jmp (ptr)`) to
//! enumerate the statically-known set of possible destination addresses.
//! Addresses are lowercase hexadecimal, space-separated, with no dots.
//!
//! ```text
//! jmp (vec) ; dispatch @[ibt ff00 ff20 ff40]
//! ```
//!
//! ### Self-modifying code operand patch (`smc`)
//!
//! ```text
//! @[smc patcher_addr field_name]
//! ```
//!
//! Attached to the instruction whose operand is overwritten at run-time.
//! `patcher_addr` is the hex address of the store instruction that performs
//! the patch; `field_name` is a mnemonic for the operand being patched
//! (e.g. `imm`, `addr`, `lo`, `hi`).  Multiple `smc` tags may appear on
//! one line when several fields are independently patched.
//!
//! ```text
//! lda #$00 ; patched at runtime @[smc ff10 imm]
//! ```
//!
//! ## Examples
//!
//! ```text
//! lda #$00 ; load zero into A @[note initialisation] @[label init]
//! ```
//!
//! Mnemonic: `lda #$00`
//! Comment prose: `load zero into A  `
//! Tags: `["note initialisation", "label init"]`
//!
//! ```text
//! ; @[label data_start] raw bytes follow
//! ```
//!
//! Mnemonic: *(none)*
//! Comment prose: ` raw bytes follow`
//! Tags: `["label data_start"]`

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::Write as FmtWrite;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A tag extracted from a comment — the raw content between `@[` and `]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tag {
    /// Raw content of the tag, not including the `@[` / `]` delimiters.
    pub content: String,
}

impl Tag {
    /// Convenience: split content on the first space to get `(key, rest)`.
    /// If there is no space the whole content is the key and rest is empty.
    pub fn key_value(&self) -> (&str, &str) {
        match self.content.split_once(' ') {
            Some((k, v)) => (k, v),
            None => (self.content.as_str(), ""),
        }
    }

    /// Interpret this tag as a [`KnownTag`].  Unrecognised keys become
    /// [`KnownTag::Other`] so that round-trips never lose information.
    pub fn parse_known(&self) -> KnownTag {
        KnownTag::from_tag(self)
    }
}

// ---------------------------------------------------------------------------
// KnownTag — structured interpretation of recognised tag keys
// ---------------------------------------------------------------------------

/// Typed representation of a [`Tag`] whose key is recognised by this library.
///
/// Construct via [`Tag::parse_known`]; convert back to a raw [`Tag`] with
/// [`KnownTag::to_tag`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum KnownTag {
    /// `@[ibt addr …]` — space-separated hex addresses that are possible
    /// targets of an indirect branch at this instruction.
    ///
    /// An empty list is valid and means "indirect branch, targets unknown".
    IndirectBranchTargets(Vec<u64>),

    /// `@[smc patcher_addr field_name]` — the named operand field of this
    /// instruction is overwritten at run-time by a self-modifying store
    /// located at `patcher`.
    ///
    /// `field` is a short mnemonic chosen by the annotator (e.g. `imm`,
    /// `addr`, `lo`, `hi`).
    SelfModifyingCode { patcher: u64, field: String },

    /// Any tag whose key is not recognised by this version of the library.
    /// The key and the rest of the content are preserved verbatim so that
    /// files containing future tag kinds round-trip without data loss.
    Other { key: String, rest: String },
}

impl KnownTag {
    /// Interpret a raw [`Tag`] into a `KnownTag`.
    pub fn from_tag(tag: &Tag) -> Self {
        let (key, rest) = tag.key_value();
        match key {
            "ibt" => {
                let tokens: Vec<&str> = rest.split_whitespace().collect();
                let addrs: Vec<u64> = tokens
                    .iter()
                    .filter_map(|s| u64::from_str_radix(s, 16).ok())
                    .collect();
                // Only accept if every token parsed successfully.
                if addrs.len() == tokens.len() {
                    KnownTag::IndirectBranchTargets(addrs)
                } else {
                    KnownTag::Other {
                        key: key.to_string(),
                        rest: rest.to_string(),
                    }
                }
            }
            "smc" => {
                // rest = "<patcher_hex> <field_name>"
                match rest.split_once(' ') {
                    Some((patcher_str, field)) if !field.is_empty() => {
                        match u64::from_str_radix(patcher_str, 16) {
                            Ok(patcher) => KnownTag::SelfModifyingCode {
                                patcher,
                                field: field.to_string(),
                            },
                            Err(_) => KnownTag::Other {
                                key: key.to_string(),
                                rest: rest.to_string(),
                            },
                        }
                    }
                    _ => KnownTag::Other {
                        key: key.to_string(),
                        rest: rest.to_string(),
                    },
                }
            }
            _ => KnownTag::Other {
                key: key.to_string(),
                rest: rest.to_string(),
            },
        }
    }

    /// Serialise back to a raw [`Tag`] that round-trips through
    /// [`KnownTag::from_tag`].
    pub fn to_tag(&self) -> Tag {
        let mut content = String::new();
        match self {
            KnownTag::IndirectBranchTargets(addrs) => {
                content.push_str("ibt");
                for a in addrs {
                    let _ = write!(content, " {:x}", a);
                }
            }
            KnownTag::SelfModifyingCode { patcher, field } => {
                let _ = write!(content, "smc {:x} {}", patcher, field);
            }
            KnownTag::Other { key, rest } => {
                content.push_str(key);
                if !rest.is_empty() {
                    content.push(' ');
                    content.push_str(rest);
                }
            }
        }
        Tag { content }
    }
}

/// The structured decomposition of a [`ListingEntry::text`] string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LineText {
    /// Instruction mnemonic (text before `;`), `None` if the line starts with
    /// `;` or has no comment separator at all and the mnemonic is empty.
    pub mnemonic: Option<String>,
    /// Free-text comment prose with all `@[…]` tags removed, `None` if there
    /// was no `;` separator.
    pub comment: Option<String>,
    /// Tags found inside the comment, in source order.
    pub tags: Vec<Tag>,
}

impl LineText {
    /// Return an iterator that interprets every raw [`Tag`] as a [`KnownTag`].
    pub fn known_tags(&self) -> impl Iterator<Item = KnownTag> + '_ {
        self.tags.iter().map(|t| t.parse_known())
    }

    /// Collect all possible indirect-branch target addresses declared by
    /// `@[ibt …]` tags on this line, in source order.
    ///
    /// Returns an empty `Vec` when no `ibt` tags are present.
    pub fn indirect_branch_targets(&self) -> Vec<u64> {
        self.tags
            .iter()
            .filter_map(|t| match t.parse_known() {
                KnownTag::IndirectBranchTargets(addrs) => Some(addrs),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// Collect all self-modifying-code patch annotations declared by
    /// `@[smc …]` tags on this line, as `(patcher_address, field_name)` pairs
    /// in source order.
    ///
    /// Returns an empty `Vec` when no `smc` tags are present.
    pub fn smc_patches(&self) -> Vec<(u64, String)> {
        self.tags
            .iter()
            .filter_map(|t| match t.parse_known() {
                KnownTag::SelfModifyingCode { patcher, field } => Some((patcher, field)),
                _ => None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a raw `text` string (the `text` field of a [`ListingEntry`]) into a
/// [`LineText`].
///
/// The function is infallible: any string that cannot be decomposed more
/// precisely is returned with the whole input as the mnemonic and no comment
/// or tags.
pub fn parse_line_text(text: &str) -> LineText {
    match text.split_once(';') {
        None => {
            // No semicolon — the entire thing is a mnemonic (or empty).
            let m = text.trim();
            LineText {
                mnemonic: if m.is_empty() { None } else { Some(m.to_string()) },
                comment: None,
                tags: Vec::new(),
            }
        }
        Some((before, after)) => {
            let m = before.trim();
            let (comment, tags) = parse_comment(after);
            LineText {
                mnemonic: if m.is_empty() { None } else { Some(m.to_string()) },
                comment: Some(comment),
                tags,
            }
        }
    }
}

/// Split a comment string (the part *after* the `;`) into plain prose and a
/// list of [`Tag`]s.  Tags are extracted in order; the prose is the comment
/// with all `@[…]` spans removed (surrounding text is kept as-is, including
/// the whitespace that surrounded the tag).
fn parse_comment(s: &str) -> (String, Vec<Tag>) {
    let mut prose = String::new();
    let mut tags: Vec<Tag> = Vec::new();
    let mut rest = s;

    loop {
        match rest.find("@[") {
            None => {
                prose.push_str(rest);
                break;
            }
            Some(tag_start) => {
                // Everything before `@[` is prose.
                prose.push_str(&rest[..tag_start]);
                rest = &rest[tag_start + 2..]; // skip "@["
                match rest.find(']') {
                    None => {
                        // Unterminated tag — treat the remainder as prose.
                        prose.push_str("@[");
                        prose.push_str(rest);
                        break;
                    }
                    Some(tag_end) => {
                        tags.push(Tag {
                            content: rest[..tag_end].to_string(),
                        });
                        rest = &rest[tag_end + 1..]; // skip "]"
                    }
                }
            }
        }
    }

    (prose, tags)
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

/// Serialise a [`LineText`] back into the flat string stored in
/// [`ListingEntry::text`].
///
/// Tags are appended at the **end** of the comment in source order, each
/// preceded by a single space.  If there are tags but no comment prose,
/// the comment section is still emitted (as a sequence of space-separated
/// tags after the `;`).
pub fn emit_line_text(lt: &LineText) -> String {
    let mut out = String::new();

    if let Some(m) = &lt.mnemonic {
        out.push_str(m);
    }

    if lt.comment.is_some() || !lt.tags.is_empty() {
        if lt.mnemonic.is_some() {
            out.push(' ');
        }
        out.push(';');
        if let Some(c) = &lt.comment {
            out.push_str(c);
        }
        for tag in &lt.tags {
            out.push_str(" @[");
            out.push_str(&tag.content);
            out.push(']');
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::*;

    #[test]
    fn no_semicolon_is_mnemonic_only() {
        let lt = parse_line_text("lda #$00");
        assert_eq!(lt.mnemonic.as_deref(), Some("lda #$00"));
        assert!(lt.comment.is_none());
        assert!(lt.tags.is_empty());
    }

    #[test]
    fn pure_annotation_line() {
        let lt = parse_line_text("; @[label entry] some prose");
        assert!(lt.mnemonic.is_none());
        assert_eq!(lt.comment.as_deref(), Some("  some prose"));
        assert_eq!(lt.tags, vec![Tag { content: "label entry".to_string() }]);
    }

    #[test]
    fn mnemonic_and_comment_with_tags() {
        let lt = parse_line_text("lda #$00 ; load zero @[note init] @[label start]");
        assert_eq!(lt.mnemonic.as_deref(), Some("lda #$00"));
        assert_eq!(lt.comment.as_deref(), Some(" load zero  "));
        assert_eq!(lt.tags.len(), 2);
        assert_eq!(lt.tags[0].content, "note init");
        assert_eq!(lt.tags[1].content, "label start");
    }

    #[test]
    fn tag_key_value() {
        let t = Tag { content: "label entry_point".to_string() };
        assert_eq!(t.key_value(), ("label", "entry_point"));
        let t2 = Tag { content: "marker".to_string() };
        assert_eq!(t2.key_value(), ("marker", ""));
    }

    #[test]
    fn roundtrip_with_tags() {
        let original = "sta $d020 ; set border colour @[note pal] @[ref d020]";
        let lt = parse_line_text(original);
        let emitted = emit_line_text(&lt);
        // Re-parse the emitted form and check structural equality.
        let lt2 = parse_line_text(&emitted);
        assert_eq!(lt.mnemonic, lt2.mnemonic);
        assert_eq!(lt.tags, lt2.tags);
    }

    #[test]
    fn unterminated_tag_treated_as_prose() {
        let lt = parse_line_text("; @[unclosed");
        assert!(lt.tags.is_empty());
        assert_eq!(lt.comment.as_deref(), Some(" @[unclosed"));
    }

    #[test]
    fn empty_string() {
        let lt = parse_line_text("");
        assert!(lt.mnemonic.is_none());
        assert!(lt.comment.is_none());
        assert!(lt.tags.is_empty());
    }

    // -----------------------------------------------------------------------
    // KnownTag — ibt
    // -----------------------------------------------------------------------

    #[test]
    fn ibt_parse_single_addr() {
        let t = Tag { content: "ibt ff00".to_string() };
        assert_eq!(t.parse_known(), KnownTag::IndirectBranchTargets(vec![0xff00]));
    }

    #[test]
    fn ibt_parse_multiple_addrs() {
        let t = Tag { content: "ibt ff00 ff20 ff40".to_string() };
        assert_eq!(
            t.parse_known(),
            KnownTag::IndirectBranchTargets(vec![0xff00, 0xff20, 0xff40])
        );
    }

    #[test]
    fn ibt_parse_empty() {
        let t = Tag { content: "ibt".to_string() };
        assert_eq!(t.parse_known(), KnownTag::IndirectBranchTargets(vec![]));
    }

    #[test]
    fn ibt_parse_invalid_addr_falls_back_to_other() {
        let t = Tag { content: "ibt ff00 xyz".to_string() };
        assert!(matches!(t.parse_known(), KnownTag::Other { .. }));
    }

    #[test]
    fn ibt_roundtrip() {
        let original = KnownTag::IndirectBranchTargets(vec![0xff00, 0xff20, 0x1000]);
        let tag = original.to_tag();
        assert_eq!(tag.parse_known(), original);
    }

    #[test]
    fn ibt_in_line_text() {
        let lt = parse_line_text("jmp (vec) ; dispatch @[ibt ff00 ff20 ff40]");
        assert_eq!(lt.mnemonic.as_deref(), Some("jmp (vec)"));
        assert_eq!(lt.indirect_branch_targets(), vec![0xff00u64, 0xff20, 0xff40]);
        assert!(lt.smc_patches().is_empty());
    }

    // -----------------------------------------------------------------------
    // KnownTag — smc
    // -----------------------------------------------------------------------

    #[test]
    fn smc_parse() {
        let t = Tag { content: "smc ff10 imm".to_string() };
        assert_eq!(
            t.parse_known(),
            KnownTag::SelfModifyingCode { patcher: 0xff10, field: "imm".to_string() }
        );
    }

    #[test]
    fn smc_parse_multi_word_field() {
        let t = Tag { content: "smc ff10 hi byte".to_string() };
        assert_eq!(
            t.parse_known(),
            KnownTag::SelfModifyingCode { patcher: 0xff10, field: "hi byte".to_string() }
        );
    }

    #[test]
    fn smc_parse_missing_field_falls_back_to_other() {
        let t = Tag { content: "smc ff10".to_string() };
        assert!(matches!(t.parse_known(), KnownTag::Other { .. }));
    }

    #[test]
    fn smc_parse_invalid_patcher_falls_back_to_other() {
        let t = Tag { content: "smc xyz imm".to_string() };
        assert!(matches!(t.parse_known(), KnownTag::Other { .. }));
    }

    #[test]
    fn smc_roundtrip() {
        let original = KnownTag::SelfModifyingCode {
            patcher: 0xff10,
            field: "imm".to_string(),
        };
        let tag = original.to_tag();
        assert_eq!(tag.parse_known(), original);
    }

    #[test]
    fn smc_in_line_text() {
        let lt = parse_line_text("lda #$00 ; patched @[smc ff10 imm]");
        assert_eq!(lt.smc_patches(), vec![(0xff10u64, "imm".to_string())]);
        assert!(lt.indirect_branch_targets().is_empty());
    }

    #[test]
    fn multiple_smc_tags() {
        let lt = parse_line_text("lda addr ; @[smc ff10 lo] @[smc ff11 hi]");
        let patches = lt.smc_patches();
        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0], (0xff10, "lo".to_string()));
        assert_eq!(patches[1], (0xff11, "hi".to_string()));
    }

    #[test]
    fn mixed_known_and_unknown_tags() {
        let lt = parse_line_text(
            "jmp (vec) ; @[label dispatch] @[ibt ff00 ff20] @[note see map]",
        );
        let known: Vec<KnownTag> = lt.known_tags().collect();
        assert!(matches!(&known[0], KnownTag::Other { key, .. } if key == "label"));
        assert_eq!(known[1], KnownTag::IndirectBranchTargets(vec![0xff00, 0xff20]));
        assert!(matches!(&known[2], KnownTag::Other { key, .. } if key == "note"));
    }

    #[test]
    fn other_tag_roundtrip() {
        let t = Tag { content: "note some free text".to_string() };
        let known = t.parse_known();
        assert_eq!(known.to_tag(), t);
    }

    #[test]
    fn other_tag_no_value_roundtrip() {
        let t = Tag { content: "marker".to_string() };
        let known = t.parse_known();
        assert_eq!(known.to_tag(), t);
    }
}
