//! `kv_asm!` and `kv_global_asm!` are pure token-formatters: their output is fed verbatim
//! into `core::arch::asm!(template, …, options(raw))` and `core::arch::global_asm!(template,
//! …, options(raw))` respectively. So proving "kv_asm! ≡ asm!" reduces to proving that
//! the *template string* the macro builds is character-for-character identical to the
//! template string a human would type inside the corresponding `asm!` invocation.
//!
//! `kv_asm_array!` exposes that exact template (split by line). The tests in this file
//! pin down the formatter on every operand shape the program uses, organised by mnemonic
//! class. Together with the `program/` end-to-end test (which actually runs the bytes
//! produced by `kv_asm!` through mollusk/sbpf), this is as strong an equivalence proof
//! as the host side can give: line-by-line textual equality of templates + executable
//! equality of the binary the templates assemble into.
//!
//! Anything outside this file's coverage that the formatter still needs to handle (new
//! mnemonic, new operand syntax) should grow a `class_*` test below before it lands in
//! `program/src/lib.rs`.

use kv_asm::kv_asm_array;

/// Convenience: assert `kv_asm_array!(...)` produces exactly these lines (in order).
fn assert_eq_lines(actual: &[&str], expected: &[&str]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "line count mismatch:\n  actual:   {actual:?}\n  expected: {expected:?}",
    );
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "line {i} differs");
    }
}

// -- 64-bit ALU, register form --------------------------------------------------------------

#[test]
fn class_alu64_reg() {
    let lines = kv_asm_array!(
        add64 r3, r4
        sub64 r3, r4
        or64 r3, r4
        and64 r3, r4
        lsh64 r3, r4
        rsh64 r3, r4
        xor64 r3, r4
        mov64 r3, r4
        arsh64 r3, r4
    );
    assert_eq_lines(
        lines,
        &[
            "add64 r3, r4",
            "sub64 r3, r4",
            "or64 r3, r4",
            "and64 r3, r4",
            "lsh64 r3, r4",
            "rsh64 r3, r4",
            "xor64 r3, r4",
            "mov64 r3, r4",
            "arsh64 r3, r4",
        ],
    );
}

// -- 64-bit ALU, immediate form -------------------------------------------------------------

#[test]
fn class_alu64_imm() {
    let lines = kv_asm_array!(
        add64 r3, 2
        sub64 r3, 2
        or64 r3, 2
        and64 r3, 2
        lsh64 r3, 2
        rsh64 r3, 2
        xor64 r3, 2
        mov64 r3, 2
        arsh64 r3, 2
        hor64 r3, 0x10000
        mov64 r3, -1
        mov64 r3, 0x44556678
    );
    assert_eq_lines(
        lines,
        &[
            "add64 r3, 2",
            "sub64 r3, 2",
            "or64 r3, 2",
            "and64 r3, 2",
            "lsh64 r3, 2",
            "rsh64 r3, 2",
            "xor64 r3, 2",
            "mov64 r3, 2",
            "arsh64 r3, 2",
            "hor64 r3, 0x10000",
            "mov64 r3, -1",
            "mov64 r3, 0x44556678",
        ],
    );
}

// -- 32-bit ALU (LLVM uses `w` register view) ----------------------------------------------

#[test]
fn class_alu32() {
    let lines = kv_asm_array!(
        add32 w3, w4
        sub32 w3, 1
        mov32 w3, 0x12345678
        arsh32 w3, w4
        lsh32 w3, 1
    );
    assert_eq_lines(
        lines,
        &[
            "add32 w3, w4",
            "sub32 w3, 1",
            "mov32 w3, 0x12345678",
            "arsh32 w3, w4",
            "lsh32 w3, 1",
        ],
    );
}

// -- PQR (v2) -------------------------------------------------------------------------------

#[test]
fn class_pqr() {
    let lines = kv_asm_array!(
        lmul64 r3, r4
        uhmul64 r3, r4
        shmul64 r3, r4
        udiv64 r3, r4
        urem64 r3, r4
        sdiv64 r3, r4
        srem64 r3, r4
        lmul32 w3, w4
        udiv32 w3, 2
    );
    assert_eq_lines(
        lines,
        &[
            "lmul64 r3, r4",
            "uhmul64 r3, r4",
            "shmul64 r3, r4",
            "udiv64 r3, r4",
            "urem64 r3, r4",
            "sdiv64 r3, r4",
            "srem64 r3, r4",
            "lmul32 w3, w4",
            "udiv32 w3, 2",
        ],
    );
}

// -- Memory: stores -------------------------------------------------------------------------

#[test]
fn class_stores_imm() {
    let lines = kv_asm_array!(
        stb [r10 - 24], 0x42
        sth [r10 - 26], 0x1234
        stw [r10 - 28], 0x12345678
        stdw [r10 - 36], 0x12345678
    );
    assert_eq_lines(
        lines,
        &[
            "stb [r10-24], 0x42",
            "sth [r10-26], 0x1234",
            "stw [r10-28], 0x12345678",
            "stdw [r10-36], 0x12345678",
        ],
    );
}

#[test]
fn class_stores_reg() {
    let lines = kv_asm_array!(
        stxb [r10 - 44], r3
        stxh [r10 - 46], r3
        stxw [r10 - 48], r3
        stxdw [r10 - 56], r3
    );
    assert_eq_lines(
        lines,
        &[
            "stxb [r10-44], r3",
            "stxh [r10-46], r3",
            "stxw [r10-48], r3",
            "stxdw [r10-56], r3",
        ],
    );
}

// -- Memory: loads --------------------------------------------------------------------------

#[test]
fn class_loads() {
    let lines = kv_asm_array!(
        ldxb r4, [r3 + 0]
        ldxh r4, [r3 + 0]
        ldxw r4, [r3 + 0]
        ldxdw r4, [r3 + -32]
        ldxdw r4, [r3 - 32]
    );
    assert_eq_lines(
        lines,
        &[
            "ldxb r4, [r3+0]",
            "ldxh r4, [r3+0]",
            "ldxw r4, [r3+0]",
            "ldxdw r4, [r3+-32]",
            "ldxdw r4, [r3-32]",
        ],
    );
}

// -- Jumps: 64-bit (LLVM only accepts the base spelling) -----------------------------------

#[test]
fn class_jumps_reg() {
    let lines = kv_asm_array!(
        jeq r3, r4, +1
        jne r3, r4, +1
        jgt r3, r4, +1
        jge r3, r4, +1
        jlt r3, r4, +1
        jle r3, r4, +1
        jsgt r3, r4, +1
        jsge r3, r4, +1
        jslt r3, r4, +1
        jsle r3, r4, +1
        ja +1
        ja -3
    );
    assert_eq_lines(
        lines,
        &[
            "jeq r3, r4, +1",
            "jne r3, r4, +1",
            "jgt r3, r4, +1",
            "jge r3, r4, +1",
            "jlt r3, r4, +1",
            "jle r3, r4, +1",
            "jsgt r3, r4, +1",
            "jsge r3, r4, +1",
            "jslt r3, r4, +1",
            "jsle r3, r4, +1",
            "ja +1",
            "ja -3",
        ],
    );
}

#[test]
fn class_jumps_imm_and_label() {
    let lines = kv_asm_array!(
        jeq r3, 0, +1
        jne r3, 0xff, -2
        ja my_label
        jne r3, 0x8, +37
    );
    assert_eq_lines(
        lines,
        &[
            "jeq r3, 0, +1",
            "jne r3, 0xff, -2",
            "ja my_label",
            "jne r3, 0x8, +37",
        ],
    );
}

// -- Endian (v2 keeps `be*`; `le*` is rejected by the verifier but the formatter still emits)

#[test]
fn class_endian() {
    let lines = kv_asm_array!(
        be16 r3
        be32 r3
        be64 r3
    );
    assert_eq_lines(lines, &["be16 r3", "be32 r3", "be64 r3"]);
}

// -- Calls / labels -------------------------------------------------------------------------

#[test]
fn class_calls_and_labels() {
    let lines = kv_asm_array!(
        call sol_log_
        call cal_a
        callx r9
        cal_a:
        mov64 r0, 0
        exit
        past_cal_a:
        ja past_cal_a
        syscall abort
    );
    assert_eq_lines(
        lines,
        &[
            "call sol_log_",
            "call cal_a",
            "callx r9",
            "cal_a:",
            "mov64 r0, 0",
            "exit",
            "past_cal_a:",
            "ja past_cal_a",
            "syscall abort",
        ],
    );
}

// -- Assembler directives (used by `kv_global_asm!` to define entrypoint) ------------------

#[test]
fn class_directives_for_global_asm() {
    let lines = kv_asm_array!(
        .globl entrypoint
        .type entrypoint, @function
        .section .text
        entrypoint:
        ldxdw r1, [r2 - 8]
        call process
        exit
    );
    assert_eq_lines(
        lines,
        &[
            ".globl entrypoint",
            ".type entrypoint, @function",
            ".section .text",
            "entrypoint:",
            "ldxdw r1, [r2-8]",
            "call process",
            "exit",
        ],
    );
}

// -- The whole program-style block (mirrors the body of `program::process`) ----------------
//
// This is the "end-to-end" host-side check: feed in a representative slice of the museum
// and confirm the macro emits exactly the strings a hand-written `asm!` template would
// contain. If a future edit to the formatter ever changes whitespace, this test breaks.

#[test]
fn end_to_end_museum_excerpt() {
    let lines = kv_asm_array!(
        stxdw [r10 - 8], r1
        stxdw [r10 - 16], r2
        call cal_a
        mov64 r7, 1
        ja past_cal_a
        cal_a:
        mov64 r0, 0
        exit
        past_cal_a:
        mov64 r3, 0x44556678
        lsh64 r3, 32
        or64 r3, 0x11223344
        add64 r3, r4
        hor64 r3, 0x10000
        lmul64 r3, r4
        sdiv64 r3, r4
        mov32 w3, 0x12345678
        add32 w3, w4
        ldxdw r1, [r10 - 8]
        ldxdw r2, [r10 - 16]
        call sol_log_
    );

    let expected = &[
        "stxdw [r10-8], r1",
        "stxdw [r10-16], r2",
        "call cal_a",
        "mov64 r7, 1",
        "ja past_cal_a",
        "cal_a:",
        "mov64 r0, 0",
        "exit",
        "past_cal_a:",
        "mov64 r3, 0x44556678",
        "lsh64 r3, 32",
        "or64 r3, 0x11223344",
        "add64 r3, r4",
        "hor64 r3, 0x10000",
        "lmul64 r3, r4",
        "sdiv64 r3, r4",
        "mov32 w3, 0x12345678",
        "add32 w3, w4",
        "ldxdw r1, [r10-8]",
        "ldxdw r2, [r10-16]",
        "call sol_log_",
    ];
    assert_eq_lines(lines, expected);

    // The actual `asm!` template that `kv_asm!` would build is the joined form. This is
    // exactly what `core::arch::asm!(template, …, options(raw))` receives — verbatim.
    let template = lines.join("\n");
    assert_eq!(template, expected.join("\n"));
}
