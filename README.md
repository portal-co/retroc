# retroc

Retro AOT compiler core, used in `moond` and for direct use. Licensed MPL-2.0.

This is a Cargo workspace containing two crates that together provide: (1) a
`no_std`-compatible file format and listing parser/emitter for retro-architecture
binary analysis, and (2) a code-generation back-end targeting a 6502-like
architecture called `hopper65`.

---

## Workspace layout

```
retroc/
  Cargo.toml                              workspace root
  crates/
    host/util/retroc-file/                listing file format library
    target/arch/hopper65/                 hopper65 code-generation back-end
  packages/                               (reserved, currently empty)
```

---

## Crate: `retroc-file`

**Path:** `crates/host/util/retroc-file`
**Edition:** 2024
**Dependencies:** [`nom`](https://crates.io/crates/nom) 8 (alloc feature, no std)

A `#![no_std]` library (requires `alloc`) that defines:

- A pluggable file registry for parsing multi-architecture binary listing files.
- Two listing formats: *attached* (ASM) and *detached*.
- A structured annotation layer for per-line metadata tags embedded in comments.

All submodules of `listing` are re-exported at the `listing` module level via
`pub use annotation::*`, `pub use asm::*`, `pub use core::*`, and
`pub use detached::*`.

### File registry (`lib.rs`)

The top-level API is `FileRegistry<'a, T, Err>`. It holds a `BTreeMap` keyed by
architecture name string, mapping to `dyn FileParser<T, Err>` trait objects.

**`FileParser<T, Err>`** — parse one architecture's section from a combined
file. The `from_bytes_and_meta` method receives:

- `bytes: &[u8]` — the raw binary blob (for detached listings)
- `meta: &str` — the textual metadata/listing content
- `registry: &FileRegistry<…>` — for recursive dispatch

Returns `Result<(&[u8], &str, T), nom::Err<Err>>` (remaining bytes, remaining
meta, parsed value).

**`ByteMetaParser<T, Err>`** — a simpler variant without registry access. Its
`from_bytes_and_meta` takes only `bytes` and `meta` (no registry) and returns
the same result shape. Used by `DetachedParser`.

**`FileRegistry::parse_file`** iterates the metadata, reading `arch <name>\n`
prefix directives to dispatch to the registered parser for that architecture.
Multiple architectures can appear in sequence in the same file. The method
returns `Vec<T>` — one item per `arch` section found.

**`ErrorKind`** is a `#[non_exhaustive]` enum with three variants:
- `ArchIncomplete` — `arch` keyword with no following newline
- `NoParser` — no parser registered for the named architecture
- `Nom(nom::error::ErrorKind)` — underlying nom parse error

It implements `Display` and `nom::error::ParseError` so it can be used directly
as a nom error type.

### Listing core (`listing/core.rs`)

Configurable listing format described by `ListingConfig`:

| Field | Meaning |
|---|---|
| `base` | Numeric base: `8` (octal) or `16` (hex) |
| `addr_groups` | Number of dot-separated groups in an address |
| `addr_group_width` | Digits per address group |
| `entry_groups` | Number of dot-separated groups in a data entry |
| `entry_group_width` | Digits per entry group |

Constructors: `ListingConfig::new_hex(…)` and `ListingConfig::new_octal(…)`.

**`ListingEntry`** holds `address: u64`, `bytes: Vec<u8>`, and `text: String`.
It implements `Display` as `{addr:08x}: {text}`. Call `entry.annotation()` to
parse the text field into a structured `LineText` via `parse_line_text`.

Free functions:

- `parse_dotted_groups` — nom parser for dotted hex/octal group notation into `u128`
- `parse_grouped_number` — non-nom variant; takes an optional expected group count
- `write_grouped_number` / `format_grouped_number` — emit dotted groups from a `u128`
- `grouped_value_to_bytes` — convert a grouped numeric value to big-endian `Vec<u8>`

### Attached listing format (`listing/asm.rs`)

`AsmParser` implements `FileParser<Vec<ListingEntry>, Err>`. Each line of the
metadata text follows the format:

```
<addr_groups> <entry_groups> <text>
```

where addresses and entries are dot-separated hex or octal groups per
`ListingConfig`. Lines must be terminated with `\n`.

Public functions:

- `parse_asm_listing(text, cfg)` — parse a full listing text into `IResult<&str, Vec<ListingEntry>, E>`
- `write_asm_listing(w, entries, cfg)` — emit entries as formatted listing lines into a `Write` implementor; returns `core::fmt::Result`
- `print_asm_listing(entries, cfg)` — same, returns `String`

### Detached listing format (`listing/detached.rs`)

`DetachedParser` implements `ByteMetaParser<Vec<ListingEntry>, Err>`. The `Err`
bound requires `nom::error::ParseError` for both `&str` and `&[u8]`. In a
detached listing the raw bytes live in the `bytes` blob while the metadata text
contains per-address annotations. An address field of `_` or `.` means "infer
from previous cursor position".

Public functions:

- `parse_detached_listing(raw, meta, cfg)` — produce
  `Result<(&[u8], &str, Vec<ListingEntry>), nom::Err<E>>` by correlating the
  metadata text with the raw byte slice; the first element of the tuple is the
  remaining unconsumed bytes.
- `write_detached_listing(w, entries, cfg)` — emit annotation lines to a `Write`
  implementor and collect raw bytes into a `Vec<u8>`; returns
  `(core::fmt::Result, Vec<u8>)`.
- `print_detached_listing(entries, cfg)` — returns `(String, Vec<u8>)`.

### Annotation layer (`listing/annotation.rs`)

Every `ListingEntry::text` is treated as:

```
line-text = [ mnemonic ] [ ";" comment ]
```

Tags appear anywhere inside the comment using `@[content]` syntax. Tags may
not be nested. Unterminated tags are treated as plain prose so files degrade
gracefully. Unrecognised tag keys are preserved verbatim so files round-trip
without loss.

**Key types:**

- `Tag { content: String }` — raw tag content between `@[` and `]`
  - `tag.key_value() -> (&str, &str)` — split on first space
  - `tag.parse_known() -> KnownTag`
- `KnownTag` — typed interpretation of recognised tags:
  - `IndirectBranchTargets(Vec<u64>)` — `@[ibt addr …]`, space-separated hex addresses; if any token fails hex parse, falls back to `Other`
  - `SelfModifyingCode { patcher: u64, field: String }` — `@[smc patcher_addr field_name]`; `field` may contain spaces
  - `Other { key: String, rest: String }` — unrecognised key, preserved for round-trip
  - `KnownTag::to_tag(&self) -> Tag` — serialise back to a raw `Tag`
- `LineText { mnemonic: Option<String>, comment: Option<String>, tags: Vec<Tag> }` — structured result of parsing a text field
  - `line_text.known_tags()` — iterator of `KnownTag`
  - `line_text.indirect_branch_targets() -> Vec<u64>`
  - `line_text.smc_patches() -> Vec<(u64, String)>`

**Free functions:**

- `parse_line_text(text: &str) -> LineText` — infallible; falls back gracefully
- `emit_line_text(lt: &LineText) -> String` — serialise back to flat string; tags appended at end of comment in source order

**Built-in tag conventions** (by key):

| Key | Meaning |
|---|---|
| `label` | symbolic label at this address (`@[label entry]`) |
| `type` | annotated data type (`@[type u8]`) |
| `ref` | cross-reference to another address (`@[ref ffff]`) |
| `note` | free-form indexed note |
| `ibt` | indirect branch targets (`@[ibt ff00 ff20]`) |
| `smc` | self-modifying code operand patch (`@[smc ff10 imm]`) |

The annotation module has a comprehensive test suite (21 tests) covering
parsing, serialisation round-trips, `ibt`/`smc` structured access, and edge
cases (unterminated tags, empty input, mixed known/unknown tags, multi-word
`smc` field names, invalid hex addresses falling back to `Other`).

---

## Crate: `hopper65`

**Path:** `crates/target/arch/hopper65`
**Edition:** 2024
**Dependencies:** [`rayoff`](https://github.com/portal-co/rayoff.git) 0.1.0
**Optional feature:** `rayon` — enables `rayoff/rayon` for parallel evaluation

A `#![no_std]` code-generation back-end (requires `alloc`) targeting a
6502-like virtual architecture called `hopper65`.

### Registers (`lib.rs`)

```rust
pub enum Reg { A, X, Y, S, P }
```

### Block code generation (`block.rs`)

The module models register-level code generation as an exhaustive state-space
search over possible instruction sequences.

**`State<V>`** — a generic code-generation state parameterised over a value
type `V`:
- `regmap: BTreeMap<V, (Reg, u32)>` — maps abstract values to the register
  holding them and the instruction index that last defined them
- `insts: Vec<Inst>` — the instruction sequence being built

**`Inst`** — the instruction set:

| Variant | Description |
|---|---|
| `StoreArg { reg, fwd }` | save a register's value for use `fwd` instructions later |
| `LoadConst { reg, value }` | load an 8-bit immediate into a register |
| `Transfer { from, to }` | register-to-register copy |
| `Unary { op, reg }` | in-place unary operation |
| `Binary { op, reg, imm }` | binary operation with 8-bit immediate |
| `Flag { flag, value }` | set or clear a processor flag |
| `Stack { op, reg }` | push or pop a register |

**`UnaryOp`:** `Inc`, `Dec`, `Asl`, `Lsr`, `Rol`, `Ror`

**`BinaryOp`:** `Adc`, `Sbc`, `And`, `Ora`, `Eor`, `Cmp`

**`Flag`:** `Carry`, `Decimal`, `Interrupt`, `Overflow`

**`StackOp`:** `Push`, `Pop`

**`Op<V>`** — the abstract operations that can be applied to produce a new value:

```rust
pub enum Op<V> {
    Just(V),   // copy/move an existing value
    Const(u8), // load a constant
    Add(V, V), Sub(V, V), And(V, V), Ora(V, V), Eor(V, V),
    Not(V), Shl(V), Shr(V), Rol(V), Ror(V),
    // all arithmetic/logical variants are stubs returning empty set (TODO)
}
```

**`State::on(this, op) -> BTreeSet<State<V>>`** is the core code-generation
method. It returns the set of all possible `State` values that correctly
implement `op`, yielding `this` as the result. This drives exhaustive register
allocation search:

- `Op::Just(v)` — if the source value is live in a register and the register
  won't be clobbered before use, alias it directly; otherwise transfer it to
  any of A/X/Y, producing one state per possible target register.
- `Op::Const(a)` — emit `LoadConst` into any of A/X/Y, producing three
  candidate states.
- All other `Op` variants return the empty set (not yet implemented).

**`State::add_patch`** inserts a `StoreArg` at a given instruction index to
save a register before a subsequent clobber, then appends a `LoadConst` as
placeholder, and fixes up all forward distances and regmap indices. This is
used internally to handle live-range conflicts discovered during instruction
selection.

**`State::sets_at(lim, reg) -> bool`** scans backwards from the end of `insts`
to `lim` to determine whether any instruction in that range writes `reg`. Used
to detect register clobbers that would require a save/restore patch.

---

## Workspace dependencies

All declared in `[workspace.dependencies]` in the root `Cargo.toml`:

| Crate | Source |
|---|---|
| `px-llvm-codegen-utils-core` / `…-info` | [portal-co/llvm-codegen-utils](https://github.com/portal-co/llvm-codegen-utils.git) |
| `portal-pc-asm-common` | [portal-co/asm-common](https://github.com/portal-co/asm-common.git) |
| `portal-solutions-asm-x86-64` / `…-riscv64` / `…-jvm` / `…-regalloc` | [portal-co/asm-arch](https://github.com/portal-co/asm-arch.git) |
| `portal-solutions-mos6502-model` | [portal-co/mx6502](https://github.com/portal-co/mx6502.git) |
| `nom` | crates.io (version 8) |
| `rayoff` | [portal-co/rayoff](https://github.com/portal-co/rayoff.git) |

Note: `px-llvm-codegen-utils-*`, `portal-pc-asm-common`, `portal-solutions-asm-*`,
and `portal-solutions-mos6502-model` are declared as workspace dependencies but
are not currently used by either crate.

---

## Building

```sh
cargo build
cargo test   # runs annotation unit tests in retroc-file
```

Enable rayon parallelism in `hopper65`:

```sh
cargo build -p hopper65 --features rayon
```

The workspace uses Cargo resolver version 3 (`resolver = "3"` in `Cargo.toml`)
and edition 2024 throughout, so a recent stable Rust (1.85+) is required.

---

## Status

Early / work-in-progress. The `hopper65` block code generator only handles
`Op::Const` and `Op::Just`; all arithmetic and logical ops (`Add`, `Sub`,
`And`, `Ora`, `Eor`, `Not`, `Shl`, `Shr`, `Rol`, `Ror`) return the empty
state set with a `// TODO` comment. The `packages/` directory is empty. The
workspace dependency list includes several assembler and LLVM codegen utility
crates that are not yet wired into any crate.
