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
//!
//! Tags with no recognised key are preserved transparently so that
//! forward-compatible files round-trip without data loss.
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
}
