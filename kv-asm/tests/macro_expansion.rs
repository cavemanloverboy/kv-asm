use kv_asm::kv_asm_array;

#[test]
fn test_multiline_expansion() {
    let asm_lines: &[&str] = kv_asm_array!(
        mov r0, 0
        add r1, r2
        ldxb r3, [r1 + 8]
        jeq r0, 0, +2
        exit
    );

    assert_eq!(asm_lines.len(), 5);
    assert_eq!(asm_lines[0], "mov r0, 0");
    assert_eq!(asm_lines[1], "add r1, r2");
    assert_eq!(asm_lines[2], "ldxb r3, [r1+8]");
    assert_eq!(asm_lines[3], "jeq r0, 0, +2");
    assert_eq!(asm_lines[4], "exit");
}

#[test]
fn test_single_instruction() {
    let asm_lines: &[&str] = kv_asm_array!(exit);
    assert_eq!(asm_lines.len(), 1);
    assert_eq!(asm_lines[0], "exit");
}

#[test]
fn test_jump_offsets() {
    let asm_lines: &[&str] = kv_asm_array!(
        ja +5
        ja -3
        jne r1, 0x8, +10
    );

    assert_eq!(asm_lines.len(), 3);
    assert_eq!(asm_lines[0], "ja +5");
    assert_eq!(asm_lines[1], "ja -3");
    assert_eq!(asm_lines[2], "jne r1, 0x8, +10");
}

#[test]
fn test_memory_operations() {
    let asm_lines: &[&str] = kv_asm_array!(
        ldxb r2, [r1 + 12]
        ldxh r3, [r1 - 8]
        stxw [r10 - 4], r1
        ldxdw r4, [r1 + 0]
    );

    assert_eq!(asm_lines.len(), 4);
    assert_eq!(asm_lines[0], "ldxb r2, [r1+12]");
    assert_eq!(asm_lines[1], "ldxh r3, [r1-8]");
    assert_eq!(asm_lines[2], "stxw [r10-4], r1");
    assert_eq!(asm_lines[3], "ldxdw r4, [r1+0]");
}

#[test]
fn test_alu_ops() {
    let asm_lines: &[&str] = kv_asm_array!(
        add64 r1, 0x605
        mov64 r2, 0x32
        mov64 r1, r0
        neg64 r2
        lsh r3, 0x8
    );

    assert_eq!(asm_lines.len(), 5);
    assert_eq!(asm_lines[0], "add64 r1, 0x605");
    assert_eq!(asm_lines[1], "mov64 r2, 0x32");
    assert_eq!(asm_lines[2], "mov64 r1, r0");
    assert_eq!(asm_lines[3], "neg64 r2");
    assert_eq!(asm_lines[4], "lsh r3, 0x8");
}

#[test]
fn test_call_syscall() {
    let asm_lines: &[&str] = kv_asm_array!(
        call +5
        syscall abort
    );

    assert_eq!(asm_lines.len(), 2);
    assert_eq!(asm_lines[0], "call +5");
    assert_eq!(asm_lines[1], "syscall abort");
}

#[test]
fn test_labels() {
    let asm_lines: &[&str] = kv_asm_array!(
        ja my_label
        jeq r0, r1, other_label
    );

    assert_eq!(asm_lines.len(), 2);
    assert_eq!(asm_lines[0], "ja my_label");
    assert_eq!(asm_lines[1], "jeq r0, r1, other_label");
}

#[test]
fn test_conditional_jump_with_regs() {
    let asm_lines: &[&str] = kv_asm_array!(
        jgt r5, r4, +20
        jsgt r2, r3, -18
    );

    assert_eq!(asm_lines.len(), 2);
    assert_eq!(asm_lines[0], "jgt r5, r4, +20");
    assert_eq!(asm_lines[1], "jsgt r2, r3, -18");
}

#[test]
fn test_store_operations() {
    let asm_lines: &[&str] = kv_asm_array!(
        stb [r10 - 8], 0x42
        sth [r1 + 4], 0x1234
    );

    assert_eq!(asm_lines.len(), 2);
    assert_eq!(asm_lines[0], "stb [r10-8], 0x42");
    assert_eq!(asm_lines[1], "sth [r1+4], 0x1234");
}

#[test]
fn test_endian() {
    let asm_lines: &[&str] = kv_asm_array!(
        be16 r0
        le32 r1
        be64 r2
    );

    assert_eq!(asm_lines.len(), 3);
    assert_eq!(asm_lines[0], "be16 r0");
    assert_eq!(asm_lines[1], "le32 r1");
    assert_eq!(asm_lines[2], "be64 r2");
}

#[test]
fn test_lddw() {
    let asm_lines: &[&str] = kv_asm_array!(
        lddw r1, 0x123456789abcdef0
    );

    assert_eq!(asm_lines.len(), 1);
    assert_eq!(asm_lines[0], "lddw r1, 0x123456789abcdef0");
}

#[test]
fn test_complex_program() {
    let asm_lines: &[&str] = kv_asm_array!(
        ldxb r2, [r1 + 12]
        ldxb r3, [r1 + 13]
        lsh r3, 0x8
        or r3, r2
        mov r0, 0x0
        jne r3, 0x8, +37
        exit
    );

    assert_eq!(asm_lines.len(), 7);
    assert_eq!(asm_lines[0], "ldxb r2, [r1+12]");
    assert_eq!(asm_lines[1], "ldxb r3, [r1+13]");
    assert_eq!(asm_lines[2], "lsh r3, 0x8");
    assert_eq!(asm_lines[3], "or r3, r2");
    assert_eq!(asm_lines[4], "mov r0, 0x0");
    assert_eq!(asm_lines[5], "jne r3, 0x8, +37");
    assert_eq!(asm_lines[6], "exit");
}
