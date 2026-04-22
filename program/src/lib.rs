//! KV Hello World — logs instruction data via `sol_log_`.
//!
//! The inline asm uses **LLVM BPF** mnemonics and pseudo-C forms (see binutils BPF pseudo-C
//! syntax). Labels use `name:` (no space before `:`); the LLVM BPF assembler resolves them in
//! jumps (`ja past_cal_a`) and calls (`call cal_a`) on its own, so `kv_asm!` only has to forward
//! tokens — it doesn't compute offsets.
//!
//! Build with `cargo-build-sbf --arch=v2` (the highest arch with a prebuilt sysroot in
//! platform-tools v1.52). The body of `process` exercises **every mnemonic that the
//! [anza/sbpf](https://github.com/anza-xyz/sbpf) assembler accepts and that is valid in
//! sBPFv2** — see the `make_instruction_map` table in `sbpf/src/assembler.rs` filtered by
//! `RequisiteVerifier::verify` for `SBPFVersion::V2`.
//!
//! Mnemonics intentionally **excluded** in v2:
//!
//! * `mul*`, `div*`, `mod*` — opcodes `0x2f` / `0x3f` / `0x9f` etc. were repurposed in v2 as the
//!   new store-with-immediate / store-from-register opcodes (`stxb`, `stxh`, `stxdw`, `stw`),
//!   so writing `mul64 r3, r4` silently assembles to `stxb [r3 + 0], w4`. Use the PQR
//!   replacements (`lmul`, `udiv`, `sdiv`, `urem`, `srem`).
//! * `neg*` — same problem (`neg64` opcode `0x87` is now `stw` in v2). Synthesize as
//!   `mov rX, 0; sub rX, rY; mov rY, rX`.
//! * `lddw` — disabled in v2; LLVM emits `mov64 + hor64` for wide immediates instead.
//! * `le16` / `le32` / `le64` — disabled in v2. Use `be*` only.
//! * `*32` jump variants (`jeq32`, `jne32`, …) — these are v3+ (`enable_jmp32`).
//!
//! Mnemonics in anza/sbpf's table that LLVM's BPF assembler **does not accept** (and which
//! therefore cannot be produced via `asm!` / `kv_asm!`, only via the standalone sbpf assembler):
//!
//! * `*64`-suffixed jump aliases (`jeq64`, `jne64`, …, `jsle64`) — LLVM only accepts the base
//!   spelling, which already encodes to the v2 `JMP64` opcodes.
//! * `jset`, `jset32`, `jset64` — no LLVM mnemonic at all.
//! * `uhmul32`, `shmul32`, `hor32` — anza/sbpf accepts the names; LLVM does not.
//! * `le16` / `le32` / `le64` — disabled in v2 anyway, see above.

#![allow(unexpected_cfgs)]
#![cfg_attr(target_os = "solana", feature(asm_experimental_arch))]

#[cfg(target_os = "solana")]
use kv_asm::{kv_asm, kv_global_asm};

#[cfg(target_os = "solana")]
extern "C" {
    #[allow(dead_code)]
    fn sol_log_(message: *const u8, len: u64);
}

// Global entrypoint defined entirely in asm via `kv_global_asm!`.
//
// At entry to a Solana sBPF program the runtime sets `r2` to point at the instruction
// data, and the eight bytes immediately preceding it (`[r2 - 8]`) hold the data length
// as a `u64`. We load that length into `r1`, then tail-call `process(len, ptr)` and
// `exit` with whatever it leaves in `r0` (0 = success).
#[cfg(not(feature = "no-entrypoint"))]
#[cfg(target_os = "solana")]
kv_global_asm!(
    .globl entrypoint
    .type entrypoint, @function
    entrypoint:
    ldxdw r1, [r2 - 8]
    call process
    exit
);

/// Logs `ptr[..len]` via `sol_log_` after running a wide mix of LLVM-legal sBPFv2
/// instructions. `extern "C"` so the asm stub above can `call process` by symbol;
/// `#[no_mangle]` so the symbol name is exactly `process`.
#[cfg(target_os = "solana")]
#[no_mangle]
pub extern "C" fn process(len: u64, ptr: *const u8) -> u64 {
    kv_asm!(
        // store ptr, len to data for later use
        stxdw [r10 - 8], r1
        stxdw [r10 - 16], r2

        // start museum of every v2 mnemonic in anza/sbpf's `make_instruction_map`

        // call (immediate) + exit + ja with named labels
        call cal_a
        mov64 r7, 1
        ja past_cal_a

        cal_a:
        mov64 r0, 0
        exit

        past_cal_a:

        // 64-bit ALU, register form
        mov64 r3, 0x44556678
        lsh64 r3, 32
        or64 r3, 0x11223344
        mov64 r4, 2
        mov64 r5, 3
        add64 r3, r4
        sub64 r3, r4
        or64 r3, r4
        and64 r3, r4
        lsh64 r3, r4
        rsh64 r3, r4
        xor64 r3, r4
        mov64 r3, r4
        arsh64 r3, r4
        hor64 r3, 0x10000
        lmul64 r3, r4
        uhmul64 r3, r4
        shmul64 r3, r4
        udiv64 r3, r4
        urem64 r3, r4
        sdiv64 r3, r4
        srem64 r3, r4

        // 64-bit ALU, immediate form
        mov64 r3, 7
        add64 r3, 2
        sub64 r3, 2
        or64 r3, 2
        and64 r3, 2
        lsh64 r3, 2
        rsh64 r3, 2
        xor64 r3, 2
        mov64 r3, 2
        arsh64 r3, 2
        hor64 r3, 2
        lmul64 r3, 2
        uhmul64 r3, 2
        shmul64 r3, 2
        udiv64 r3, 2
        urem64 r3, 2
        sdiv64 r3, 2
        srem64 r3, 2

        // synthesize negate (no `neg` in v2): r3 = -r3
        mov64 r3, 5
        mov64 r4, 0
        sub64 r4, r3
        mov64 r3, r4

        // 32-bit ALU, register form (LLVM uses `w` view for 32-bit ops)
        mov32 w3, 0x12345678
        mov32 w4, 1
        add32 w3, w4
        sub32 w3, w4
        or32 w3, w4
        and32 w3, w4
        lsh32 w3, w4
        rsh32 w3, w4
        xor32 w3, w4
        mov32 w3, w4
        arsh32 w3, w4

        // 32-bit ALU, immediate form
        mov32 w3, 7
        add32 w3, 1
        sub32 w3, 1
        or32 w3, 1
        and32 w3, 1
        lsh32 w3, 1
        rsh32 w3, 1
        xor32 w3, 1
        mov32 w3, 1
        arsh32 w3, 1

        // 32-bit PQR
        mov32 w3, 0x100
        mov32 w4, 2
        lmul32 w3, w4
        udiv32 w3, w4
        urem32 w3, w4
        sdiv32 w3, w4
        srem32 w3, w4
        lmul32 w3, 2
        udiv32 w3, 2
        urem32 w3, 2
        sdiv32 w3, 2
        srem32 w3, 2

        // stores: immediate, then register; all four widths, both classes
        stb [r10 - 24], 0x42
        sth [r10 - 26], 0x1234
        stw [r10 - 28], 0x12345678
        stdw [r10 - 36], 0x12345678
        mov64 r3, 0xAB
        stxb [r10 - 44], r3
        stxh [r10 - 46], r3
        stxw [r10 - 48], r3
        stxdw [r10 - 56], r3

        // loads: all four widths
        mov64 r3, r10
        add64 r3, -24
        ldxb r4, [r3 + 0]
        ldxh r4, [r3 + 0]
        ldxw r4, [r3 + 0]
        ldxdw r4, [r3 + -32]

        // 64-bit jumps (base spelling — produces JMP64 opcodes; LLVM rejects the `*64` aliases)
        mov64 r3, 0
        mov64 r4, 1
        jeq r3, r4, +1
        mov64 r5, r5
        jgt r3, r4, +1
        mov64 r5, r5
        jge r3, r4, +1
        mov64 r5, r5
        jlt r4, r3, +1
        mov64 r5, r5
        jle r3, r4, +1
        mov64 r5, r5
        jne r3, r4, +1
        mov64 r5, r5
        jsgt r4, r3, +1
        mov64 r5, r5
        jsge r3, r4, +1
        mov64 r5, r5
        jslt r3, r4, +1
        mov64 r5, r5
        jsle r3, r4, +1
        mov64 r5, r5
        ja +1
        mov64 r5, r5

        // callx — opcode emitted but skipped at runtime (jeq is always taken)
        mov64 r9, 0
        jeq r9, r9, +1
        callx r9

        // endian (be only — le16/le32/le64 are disabled in v2)
        mov64 r3, 0x0102
        be16 r3
        mov64 r3, 0x01020304
        be32 r3
        mov64 r3, 0x05060708
        be64 r3
        // end museum

        // print input — ptr was at [r10 - 16], len at [r10 - 8]; sol_log_ wants r1=ptr, r2=len
        ldxdw r1, [r10 - 16]
        ldxdw r2, [r10 - 8]
        call sol_log_

        inlateout("r1") len => _,
        inlateout("r2") ptr => _,
        lateout("r0") _,
        lateout("r3") _,
        lateout("r4") _,
        lateout("r5") _,
        lateout("r6") _,
        lateout("r7") _,
        lateout("r8") _,
        lateout("r9") _,
    );

    0
}

#[cfg(not(target_os = "solana"))]
pub extern "C" fn process(_len: u64, _ptr: *const u8) -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use mollusk_svm::{result::Check, Mollusk};
    use solana_instruction::Instruction;
    use solana_pubkey::Pubkey;

    #[test]
    fn test_hello_world() {
        let program_id = Pubkey::new_unique();
        std::env::set_var("SBF_OUT_DIR", "../target/deploy");
        let mollusk = Mollusk::new(&program_id, "kv_program");

        let instruction = Instruction::new_with_bytes(program_id, b"Hello World!", vec![]);

        mollusk.process_and_validate_instruction(&instruction, &[], &[Check::success()]);
    }
}
