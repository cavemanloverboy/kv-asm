# kv-asm

Quote-free inline and global sBPF assembly macros for Solana programs.

`kv-asm` provides three procedural macros — `kv_asm!`, `kv_global_asm!`, and
`kv_asm_array!` — that let you write sBPF assembly the way you'd write it in
a `.s` file (one instruction per line, no quotes, no manually concatenated
strings) and have it expand to a standard `core::arch::asm!` /
`core::arch::global_asm!` invocation.

```rust,ignore
use kv_asm::{kv_asm, kv_global_asm};

// Global entrypoint, written entirely in asm.
kv_global_asm!(
    .globl entrypoint
    .type entrypoint, @function
    entrypoint:
    ldxdw r1, [r2 - 8]   // load instruction-data length
    call process         // process(len, ptr)
    exit
);

#[no_mangle]
pub extern "C" fn process(len: u64, ptr: *const u8) -> u64 {
    kv_asm!(
        mov64 r1, ptr
        mov64 r2, len
        call sol_log_

        in("r1") ptr,
        in("r2") len,
        lateout("r0") _,
    );
    0
}
```

## Why

`core::arch::asm!` and `core::arch::global_asm!` take a sequence of
**string-literal templates**. Hand-writing those strings for non-trivial sBPF
programs is painful:

```rust,ignore
core::arch::asm!(
    "mov64 r1, r2",
    "ldxdw r3, [r1 + 8]",
    "jeq r3, 0, 2f",
    "exit",
    "2:",
    // ...
);
```

`kv-asm` lets you drop the quotes:

```rust,ignore
kv_asm!(
    mov64 r1, r2
    ldxdw r3, [r1 + 8]
    jeq r3, 0, exit_label
    exit
    exit_label:
);
```

The macro is a pure token-formatter: it parses the input into one logical line
per source line, renders each line into the canonical assembler spelling, joins
them with `\n`, and hands the resulting string straight to `core::arch::asm!`
(or `core::arch::global_asm!`) with `options(raw)`. **Labels and relocations
are resolved by the LLVM BPF assembler exactly as they would be in a `.s`
file** — `kv-asm` never computes offsets itself.

## Macros

### `kv_asm!`

Inline assembly. Wraps `core::arch::asm!(template, …, options(raw))` in an
`unsafe` block. Operands (`in(…)`, `out(…)`, `lateout(…)`, `inlateout(…)`,
`const`, `sym`, `options(…)`, `clobber_abi(…)`) are passed through verbatim
and may appear after the instructions, comma-separated.

```rust,ignore
kv_asm!(
    mov64 r0, 1
    add64 r0, r1

    inout("r1") x => _,
    lateout("r0") result,
);
```

### `kv_global_asm!`

Module-level assembly. Wraps `core::arch::global_asm!(template, …)` (no
`unsafe`, no implicit options — supply `options(raw)` yourself if you want
it). Useful for defining the program entrypoint and any other naked symbols.

```rust,ignore
kv_global_asm!(
    .globl my_helper
    my_helper:
    mov64 r0, 0
    exit
);
```

### `kv_asm_array!`

Returns the formatted instruction strings as a `&'static [&'static str]`,
without invoking any `asm!`. Primarily useful for testing — see
`tests/equivalence.rs`.

```rust,ignore
let lines: &[&str] = kv_asm_array!(
    mov64 r0, 1
    exit
);
assert_eq!(lines, &["mov64 r0, 1", "exit"]);
```

## Formatter rules

The formatter aims to match the canonical spelling produced by handwriting an
`asm!` template:

| Source                       | Emitted                |
|------------------------------|------------------------|
| `mov64 r0, 1`                | `mov64 r0, 1`          |
| `ldxdw r1, [r2 + 8]`         | `ldxdw r1, [r2+8]`     |
| `ldxdw r1, [r2 - 8]`         | `ldxdw r1, [r2-8]`     |
| `ja +5` / `ja -3`            | `ja +5` / `ja -3`      |
| `cal_a:` / `call cal_a`      | `cal_a:` / `call cal_a`|
| `.globl entrypoint`          | `.globl entrypoint`    |
| `.type entrypoint, @function`| `.type entrypoint, @function` |

Comments (`// …`) and blank source lines are stripped.

## Scope and limitations

* Targets **sBPFv0** — the version mainnet runs and the default for
  `cargo-build-sbf` (`--arch=v0`). The macro itself is version-agnostic (it's
  just a token formatter); what's "valid" is decided by LLVM's BPF assembler
  at compile time and by the on-chain verifier at load time.
* The macro emits whatever you write — it does not validate mnemonics. If you
  use a name LLVM doesn't recognize, assembly will fail at compile time with
  the LLVM error.
* Mnemonics LLVM's BPF backend **does not accept** (so unreachable from
  `kv_asm!` / `kv_global_asm!` regardless of sBPF version):
  - `*64`-suffixed jump aliases (`jeq64`, `jne64`, …, `jsle64`) — use the
    base spelling (`jeq`, `jne`, …), which already encodes to the v0 64-bit
    jump opcodes.
  - `jset`, `jset32`, `jset64`.
  - `le16`, `le32`, `le64` (the BPF VM is little-endian, so they'd be a
    no-op anyway — use `be*`).
  - `uhmul32`, `shmul32`, `hor32`.
* Mnemonics that **only exist in v2+** and will be rejected by the v0
  verifier at load time:
  - PQR family: `lmul*`, `uhmul64`, `shmul64`, `udiv*`, `urem*`, `sdiv*`,
    `srem*` (instruction class `BPF_PQR = 0x06`, gated on `enable_pqr()`).
  - `hor64` (gated on `disable_lddw()`). Use `lddw` for wide immediates.
  - The new store-from-imm/store-from-reg classes (`ST_*B_*`) which share
    bytes with `mul*` / `div*` / `neg*` / `mod*` and only kick in under
    `move_memory_instruction_classes()`.
* Mnemonics that **only exist in v3+**: `*32` jump variants (`jeq32`,
  `jne32`, …) under `enable_jmp32()`.

## Testing strategy

The `tests/` directory contains class-by-class equivalence tests
(`equivalence.rs`) that pin down the formatter's output character-for-character
against the templates a human would hand-write inside `asm!`. Combined with an
on-chain integration test of the assembled bytes, this gives a strong
end-to-end equivalence guarantee.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
