use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;

// Mnemonic inventory for tests: keep in sync with `make_instruction_map` in
// [anza/sbpf](https://github.com/anza-xyz/sbpf) (`sbpf/src/assembler.rs` in this workspace).

const ASM_OPERAND_KEYWORDS: &[&str] = &[
    "in",
    "out",
    "inout",
    "lateout",
    "inlateout",
    "const",
    "sym",
    "options",
    "clobber_abi",
];

/// Inline sBPF assembly macro — like `asm!()` but without quotes around instructions.
///
/// # Example
///
/// ```ignore
/// use kv_asm::kv_asm;
///
/// let ptr = data.as_ptr();
/// let len = data.len();
///
/// kv_asm!(
///     syscall sol_log_
///     in("r1") ptr,
///     in("r2") len,
///     lateout("r0") _,
/// );
/// ```
///
/// Expands to:
/// ```ignore
/// unsafe {
///     core::arch::asm!(
///         "syscall sol_log_",
///         in("r1") ptr,
///         in("r2") len,
///         lateout("r0") _,
///     )
/// }
/// ```
///
/// # Without operands
///
/// ```ignore
/// kv_asm!(
///     mov r0, 0
///     exit
/// );
/// ```
#[proc_macro]
pub fn kv_asm(input: TokenStream) -> TokenStream {
    let input2: proc_macro2::TokenStream = input.into();
    let tokens: Vec<TokenTree> = input2.into_iter().collect();

    if tokens.is_empty() {
        return quote! {
            unsafe {
                core::arch::asm!("", options(raw))
            }
        }
        .into();
    }

    let (asm_tokens, operand_tokens) = split_asm_and_operands(&tokens);

    let lines = group_tokens_by_line(&asm_tokens);
    let asm_strings: Vec<String> = lines
        .into_iter()
        .map(|line_tokens| format_instruction(&line_tokens))
        .filter(|s: &String| !s.is_empty())
        .collect();

    let operands: proc_macro2::TokenStream = operand_tokens.into_iter().cloned().collect();

    let template = asm_strings.join("\n");
    let template_lit = proc_macro2::Literal::string(&template);

    let output = if operands.is_empty() {
        quote! {
            unsafe {
                core::arch::asm!(
                    #template_lit,
                    options(raw)
                )
            }
        }
    } else {
        quote! {
            unsafe {
                core::arch::asm!(
                    #template_lit,
                    #operands
                    options(raw)
                )
            }
        }
    };

    output.into()
}

/// Like [`kv_asm!`] but expands to [`core::arch::global_asm!`] for module-scope assembly.
///
/// Use this to define entire functions (or other top-level symbols) in pure asm:
///
/// ```ignore
/// use kv_asm::kv_global_asm;
///
/// kv_global_asm!(
///     .globl entrypoint
///     .type entrypoint, @function
///     entrypoint:
///     ldxdw r1, [r2 - 8]   // load len from before the data ptr
///     call process         // process(len, ptr)
///     exit                 // return r0 to the runtime
/// );
/// ```
///
/// The token stream is parsed and formatted exactly the same way as [`kv_asm!`], so labels,
/// memory operands and assembler directives all share one implementation. `sym`/`const`
/// operands are passed through to `global_asm!` if present.
#[proc_macro]
pub fn kv_global_asm(input: TokenStream) -> TokenStream {
    let input2: proc_macro2::TokenStream = input.into();
    let tokens: Vec<TokenTree> = input2.into_iter().collect();

    if tokens.is_empty() {
        return quote! { core::arch::global_asm!(""); }.into();
    }

    let (asm_tokens, operand_tokens) = split_asm_and_operands(&tokens);

    let lines = group_tokens_by_line(&asm_tokens);
    let asm_strings: Vec<String> = lines
        .into_iter()
        .map(|line_tokens| format_instruction(&line_tokens))
        .filter(|s: &String| !s.is_empty())
        .collect();

    let operands: proc_macro2::TokenStream = operand_tokens.into_iter().cloned().collect();

    let template = asm_strings.join("\n");
    let template_lit = proc_macro2::Literal::string(&template);

    let output = if operands.is_empty() {
        quote! {
            core::arch::global_asm!(
                #template_lit,
                options(raw)
            );
        }
    } else {
        quote! {
            core::arch::global_asm!(
                #template_lit,
                #operands
            );
        }
    };

    output.into()
}

/// Same as `kv_asm!()` but outputs an array of string literals for testing.
///
/// # Example
///
/// ```ignore
/// let lines: &[&str] = kv_asm_array!(
///     mov r0, 0
///     add r1, r2
/// );
/// assert_eq!(lines[0], "mov r0, 0");
/// ```
#[proc_macro]
pub fn kv_asm_array(input: TokenStream) -> TokenStream {
    let input2: proc_macro2::TokenStream = input.into();
    let tokens: Vec<TokenTree> = input2.into_iter().collect();

    if tokens.is_empty() {
        return quote! { &[] as &[&str] }.into();
    }

    let (asm_tokens, _) = split_asm_and_operands(&tokens);

    let lines = group_tokens_by_line(&asm_tokens);
    let asm_strings: Vec<String> = lines
        .into_iter()
        .map(|line_tokens| format_instruction(&line_tokens))
        .filter(|s: &String| !s.is_empty())
        .collect();

    quote! {
        &[#(#asm_strings),*]
    }
    .into()
}

fn is_asm_operand_keyword(token: &TokenTree) -> bool {
    if let TokenTree::Ident(ident) = token {
        let s = ident.to_string();
        ASM_OPERAND_KEYWORDS.contains(&s.as_str())
    } else {
        false
    }
}

/// BPF pseudo-C (`r3 = le16 r3`) uses the same `ident =` shape as Rust `asm!` operands.
/// Treat only non-register idents as the start of the operand list.
fn is_bpf_plain_reg_ident(ident: &proc_macro2::Ident) -> bool {
    let s = ident_to_asm(ident);
    let b = s.as_bytes();
    if b.len() < 2 {
        return false;
    }
    if b[0] != b'r' && b[0] != b'w' {
        return false;
    }
    b[1..].iter().all(|c| c.is_ascii_digit())
}

fn is_named_operand_start(tokens: &[TokenTree], start_idx: usize) -> bool {
    if start_idx + 1 >= tokens.len() {
        return false;
    }

    if let TokenTree::Ident(ident) = &tokens[start_idx] {
        if is_bpf_plain_reg_ident(ident) {
            return false;
        }
        if let TokenTree::Punct(punct) = &tokens[start_idx + 1] {
            return punct.as_char() == '=';
        }
    }
    false
}

fn split_asm_and_operands(tokens: &[TokenTree]) -> (Vec<&TokenTree>, Vec<&TokenTree>) {
    let mut asm_tokens = Vec::new();
    let mut operand_tokens = Vec::new();
    let mut in_operands = false;
    let mut prev_line_num: Option<usize> = None;
    let mut line_start_idx: Option<usize> = None;

    for (i, token) in tokens.iter().enumerate() {
        if in_operands {
            operand_tokens.push(token);
            continue;
        }

        let line_num = token.span().start().line;
        let is_new_line = prev_line_num.map_or(true, |prev| line_num != prev);

        if is_new_line {
            line_start_idx = Some(i);
        }

        let at_line_start = line_start_idx == Some(i);

        if at_line_start && is_asm_operand_keyword(token) {
            in_operands = true;
            operand_tokens.push(token);
        } else if at_line_start && is_named_operand_start(tokens, i) {
            in_operands = true;
            operand_tokens.push(token);
        } else {
            asm_tokens.push(token);
        }

        prev_line_num = Some(line_num);
    }

    (asm_tokens, operand_tokens)
}

fn group_tokens_by_line<'a>(tokens: &[&'a TokenTree]) -> Vec<Vec<&'a TokenTree>> {
    if tokens.is_empty() {
        return vec![];
    }

    let mut lines: Vec<Vec<&'a TokenTree>> = vec![];
    let mut current_line: Vec<&'a TokenTree> = vec![];
    let mut prev_line_num: Option<usize> = None;

    for &token in tokens {
        let span = token.span();
        let line_num = span.start().line;

        if let Some(prev) = prev_line_num {
            if line_num != prev && !current_line.is_empty() {
                lines.push(current_line);
                current_line = vec![];
            }
        }

        current_line.push(token);
        prev_line_num = Some(line_num);
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

fn format_instruction(tokens: &[&TokenTree]) -> String {
    let mut parts: Vec<String> = vec![];
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];
        match token {
            TokenTree::Ident(ident) => {
                parts.push(ident_to_asm(ident));
            }
            TokenTree::Literal(lit) => {
                parts.push(lit.to_string());
            }
            TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                match ch {
                    ',' => {
                        parts.push(",".to_string());
                    }
                    '+' | '-' => {
                        if i + 1 < tokens.len() {
                            if let TokenTree::Literal(next_lit) = tokens[i + 1] {
                                parts.push(format!("{}{}", ch, next_lit));
                                i += 1;
                            } else {
                                parts.push(ch.to_string());
                            }
                        } else {
                            parts.push(ch.to_string());
                        }
                    }
                    '%' => {
                        if i + 1 < tokens.len() {
                            if let TokenTree::Punct(next) = tokens[i + 1] {
                                if next.as_char() == '=' {
                                    parts.push("%=".to_string());
                                    i += 1;
                                } else {
                                    parts.push('%'.to_string());
                                }
                            } else {
                                parts.push('%'.to_string());
                            }
                        } else {
                            parts.push('%'.to_string());
                        }
                    }
                    // Glue assembler-directive prefixes to the following ident: `.globl`,
                    // `.text`, `.type`, `@function`, `@object`, etc.
                    '.' | '@' => {
                        if i + 1 < tokens.len() {
                            if let TokenTree::Ident(next_ident) = tokens[i + 1] {
                                parts.push(format!("{}{}", ch, ident_to_asm(next_ident)));
                                i += 1;
                            } else {
                                parts.push(ch.to_string());
                            }
                        } else {
                            parts.push(ch.to_string());
                        }
                    }
                    _ => {
                        parts.push(ch.to_string());
                    }
                }
            }
            TokenTree::Group(group) => {
                let delimiter = group.delimiter();
                let inner: Vec<TokenTree> = group.stream().into_iter().collect();
                let inner_refs: Vec<&TokenTree> = inner.iter().collect();
                let inner_str = format_memory_operand(&inner_refs);

                match delimiter {
                    proc_macro2::Delimiter::Bracket => {
                        parts.push(format!("[{}]", inner_str));
                    }
                    proc_macro2::Delimiter::Brace => {
                        parts.push(format!("{{{}}}", inner_str));
                    }
                    proc_macro2::Delimiter::Parenthesis => {
                        parts.push(format!("({})", inner_str));
                    }
                    proc_macro2::Delimiter::None => {
                        parts.push(inner_str);
                    }
                }
            }
        }
        i += 1;
    }

    join_asm_parts(&parts)
}

fn format_memory_operand(tokens: &[&TokenTree]) -> String {
    let mut result = String::new();
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];
        match token {
            TokenTree::Ident(ident) => {
                result.push_str(&ident_to_asm(ident));
            }
            TokenTree::Literal(lit) => {
                result.push_str(&lit.to_string());
            }
            TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                if ch == '+' || ch == '-' {
                    if i + 1 < tokens.len() {
                        if let TokenTree::Literal(next_lit) = tokens[i + 1] {
                            result.push(ch);
                            result.push_str(&next_lit.to_string());
                            i += 1;
                        } else if let TokenTree::Ident(next_ident) = tokens[i + 1] {
                            result.push(ch);
                            result.push_str(&next_ident.to_string());
                            i += 1;
                        } else {
                            result.push(ch);
                        }
                    } else {
                        result.push(ch);
                    }
                } else {
                    result.push(ch);
                }
            }
            TokenTree::Group(group) => {
                let inner: Vec<TokenTree> = group.stream().into_iter().collect();
                let inner_refs: Vec<&TokenTree> = inner.iter().collect();
                result.push_str(&format_memory_operand(&inner_refs));
            }
        }
        i += 1;
    }

    result
}

fn ident_to_asm(ident: &proc_macro2::Ident) -> String {
    let s = ident.to_string();
    s.strip_prefix("r#").map(str::to_string).unwrap_or(s)
}

fn join_asm_parts(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }

    let mut result = String::new();
    let mut prev_needs_space = false;

    for part in parts {
        if part == "," {
            result.push_str(", ");
            prev_needs_space = false;
        } else if part.starts_with('[') || part.starts_with('{') || part.starts_with('(') {
            if prev_needs_space {
                result.push(' ');
            }
            result.push_str(part);
            prev_needs_space = false;
        } else if part.starts_with('+') || part.starts_with('-') {
            if prev_needs_space {
                result.push(' ');
            }
            result.push_str(part);
            prev_needs_space = true;
        } else if part == ":" {
            // BPF labels are `name:` (no space before `:`). If more tokens follow on the same
            // line (`name: insn`), the space after `:` comes from `prev_needs_space` on the next part.
            result.push(':');
            prev_needs_space = true;
        } else {
            if prev_needs_space {
                result.push(' ');
            }
            result.push_str(part);
            prev_needs_space = true;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;

    fn test_format(input: TokenStream) -> Vec<String> {
        let tokens: Vec<TokenTree> = input.into_iter().collect();
        let (asm_tokens, _) = split_asm_and_operands(&tokens);
        let lines = group_tokens_by_line(&asm_tokens);
        lines
            .into_iter()
            .map(|line_tokens| format_instruction(&line_tokens))
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn test_simple_exit() {
        let result = test_format(quote! { exit });
        assert_eq!(result, vec!["exit"]);
    }

    #[test]
    fn test_mov_reg_imm() {
        let result = test_format(quote! { mov r0, 0 });
        assert_eq!(result, vec!["mov r0, 0"]);
    }

    #[test]
    fn test_add_reg_reg() {
        let result = test_format(quote! { add r1, r2 });
        assert_eq!(result, vec!["add r1, r2"]);
    }

    #[test]
    fn test_memory_load() {
        let result = test_format(quote! { ldxb r2, [r1 + 12] });
        assert_eq!(result, vec!["ldxb r2, [r1+12]"]);
    }

    #[test]
    fn test_memory_negative_offset() {
        let result = test_format(quote! { ldxh r4, [r1 - 8] });
        assert_eq!(result, vec!["ldxh r4, [r1-8]"]);
    }

    #[test]
    fn test_jump_offset() {
        let result = test_format(quote! { ja +5 });
        assert_eq!(result, vec!["ja +5"]);
    }

    #[test]
    fn test_jump_negative() {
        let result = test_format(quote! { ja -3 });
        assert_eq!(result, vec!["ja -3"]);
    }

    #[test]
    fn test_conditional_jump() {
        let result = test_format(quote! { jne r3, 0x8, +37 });
        assert_eq!(result, vec!["jne r3, 0x8, +37"]);
    }

    #[test]
    fn test_hex_immediate() {
        let result = test_format(quote! { mov r0, 0x1234 });
        assert_eq!(result, vec!["mov r0, 0x1234"]);
    }

    #[test]
    fn test_label_definition_no_space_before_colon() {
        use proc_macro2::TokenStream;

        let lines: &[TokenStream] = &[
            quote!(call cal_a),
            quote!(ja past_cal_a),
            quote!(cal_a:),
            quote!(mov64 r0, 0),
            quote!(exit),
            quote!(past_cal_a:),
            quote!(mov64 r1, 1),
        ];
        let result: Vec<String> = lines
            .iter()
            .flat_map(|ts| test_format(ts.clone()))
            .collect();

        assert_eq!(
            result,
            vec![
                "call cal_a",
                "ja past_cal_a",
                "cal_a:",
                "mov64 r0, 0",
                "exit",
                "past_cal_a:",
                "mov64 r1, 1",
            ]
        );
    }

    #[test]
    fn test_label_and_insn_same_line_has_space_after_colon() {
        let result = test_format(quote!(cal_a: mov64 r0, 0));
        assert_eq!(result, vec!["cal_a: mov64 r0, 0"]);
    }

    #[test]
    fn test_directives_glue_dot_and_at_to_following_ident() {
        use proc_macro2::TokenStream;
        let lines: &[TokenStream] = &[
            quote!(.globl entrypoint),
            quote!(.type entrypoint, @function),
            quote!(.section .text),
            quote!(entrypoint:),
            quote!(ldxdw r1, [r2 - 8]),
            quote!(call process),
            quote!(exit),
        ];
        let out: Vec<String> = lines
            .iter()
            .flat_map(|ts| test_format(ts.clone()))
            .collect();
        assert_eq!(
            out,
            vec![
                ".globl entrypoint",
                ".type entrypoint, @function",
                ".section .text",
                "entrypoint:",
                "ldxdw r1, [r2-8]",
                "call process",
                "exit",
            ]
        );
    }

    /// Every instruction name registered in `make_instruction_map` (anza/sbpf
    /// `assembler.rs`), with minimal operands so the formatter still emits a valid line.
    /// `mod` is spelled `r#mod` because `mod` is a Rust keyword.
    #[test]
    fn test_sbpf_make_instruction_map_mnemonics() {
        use proc_macro2::TokenStream;

        // One `quote!` per line so each token carries a distinct `Span` line (a single
        // `quote! { ... }` block would collapse to one line for `group_tokens_by_line`).
        let lines: &[TokenStream] = &[
            quote!(lddw r1, 0x1122334455667788_u64),
            quote!(ja +1),
            quote!(syscall sol_log_),
            quote!(call callee),
            quote!(callx r5),
            quote!(exit),
            quote!(neg r3),
            quote!(neg32 r3),
            quote!(neg64 r3),
            quote!(add r3, r4),
            quote!(add32 r3, r4),
            quote!(add64 r3, r4),
            quote!(sub r3, r4),
            quote!(sub32 r3, r4),
            quote!(sub64 r3, r4),
            quote!(mul r3, r4),
            quote!(mul32 r3, r4),
            quote!(mul64 r3, r4),
            quote!(div r3, r4),
            quote!(div32 r3, r4),
            quote!(div64 r3, r4),
            quote!(or r3, r4),
            quote!(or32 r3, r4),
            quote!(or64 r3, r4),
            quote!(and r3, r4),
            quote!(and32 r3, r4),
            quote!(and64 r3, r4),
            quote!(lsh r3, r4),
            quote!(lsh32 r3, r4),
            quote!(lsh64 r3, r4),
            quote!(rsh r3, r4),
            quote!(rsh32 r3, r4),
            quote!(rsh64 r3, r4),
            quote!(r#mod r3, r4),
            quote!(mod32 r3, r4),
            quote!(mod64 r3, r4),
            quote!(xor r3, r4),
            quote!(xor32 r3, r4),
            quote!(xor64 r3, r4),
            quote!(mov r3, r4),
            quote!(mov32 r3, r4),
            quote!(mov64 r3, r4),
            quote!(arsh r3, r4),
            quote!(arsh32 r3, r4),
            quote!(arsh64 r3, r4),
            quote!(hor r3, r4),
            quote!(hor32 r3, r4),
            quote!(hor64 r3, r4),
            quote!(lmul r3, r4),
            quote!(lmul32 r3, r4),
            quote!(lmul64 r3, r4),
            quote!(uhmul r3, r4),
            quote!(uhmul64 r3, r4),
            quote!(shmul r3, r4),
            quote!(shmul64 r3, r4),
            quote!(udiv r3, r4),
            quote!(udiv32 r3, r4),
            quote!(udiv64 r3, r4),
            quote!(urem r3, r4),
            quote!(urem32 r3, r4),
            quote!(urem64 r3, r4),
            quote!(sdiv r3, r4),
            quote!(sdiv32 r3, r4),
            quote!(sdiv64 r3, r4),
            quote!(srem r3, r4),
            quote!(srem32 r3, r4),
            quote!(srem64 r3, r4),
            quote!(ldxb r2, [r1 + 0]),
            quote!(ldxh r2, [r1 + 0]),
            quote!(ldxw r2, [r1 + 0]),
            quote!(ldxdw r2, [r1 + 0]),
            quote!(stb [r10 - 8], 1),
            quote!(sth [r10 - 8], 1),
            quote!(stw [r10 - 8], 1),
            quote!(stdw [r10 - 8], 1),
            quote!(stxb [r10 - 8], r1),
            quote!(stxh [r10 - 8], r1),
            quote!(stxw [r10 - 8], r1),
            quote!(stxdw [r10 - 8], r1),
            quote!(jeq r1, r2, +1),
            quote!(jeq32 r1, r2, +1),
            quote!(jeq64 r1, r2, +1),
            quote!(jgt r1, r2, +1),
            quote!(jgt32 r1, r2, +1),
            quote!(jgt64 r1, r2, +1),
            quote!(jge r1, r2, +1),
            quote!(jge32 r1, r2, +1),
            quote!(jge64 r1, r2, +1),
            quote!(jlt r1, r2, +1),
            quote!(jlt32 r1, r2, +1),
            quote!(jlt64 r1, r2, +1),
            quote!(jle r1, r2, +1),
            quote!(jle32 r1, r2, +1),
            quote!(jle64 r1, r2, +1),
            quote!(jset r1, r2, +1),
            quote!(jset32 r1, r2, +1),
            quote!(jset64 r1, r2, +1),
            quote!(jne r1, r2, +1),
            quote!(jne32 r1, r2, +1),
            quote!(jne64 r1, r2, +1),
            quote!(jsgt r1, r2, +1),
            quote!(jsgt32 r1, r2, +1),
            quote!(jsgt64 r1, r2, +1),
            quote!(jsge r1, r2, +1),
            quote!(jsge32 r1, r2, +1),
            quote!(jsge64 r1, r2, +1),
            quote!(jslt r1, r2, +1),
            quote!(jslt32 r1, r2, +1),
            quote!(jslt64 r1, r2, +1),
            quote!(jsle r1, r2, +1),
            quote!(jsle32 r1, r2, +1),
            quote!(jsle64 r1, r2, +1),
            quote!(be16 r2),
            quote!(be32 r2),
            quote!(be64 r2),
            quote!(le16 r2),
            quote!(le32 r2),
            quote!(le64 r2),
        ];

        let out: Vec<String> = lines
            .iter()
            .flat_map(|ts| test_format(ts.clone()))
            .collect();

        assert_eq!(
            out,
            vec![
                "lddw r1, 0x1122334455667788_u64",
                "ja +1",
                "syscall sol_log_",
                "call callee",
                "callx r5",
                "exit",
                "neg r3",
                "neg32 r3",
                "neg64 r3",
                "add r3, r4",
                "add32 r3, r4",
                "add64 r3, r4",
                "sub r3, r4",
                "sub32 r3, r4",
                "sub64 r3, r4",
                "mul r3, r4",
                "mul32 r3, r4",
                "mul64 r3, r4",
                "div r3, r4",
                "div32 r3, r4",
                "div64 r3, r4",
                "or r3, r4",
                "or32 r3, r4",
                "or64 r3, r4",
                "and r3, r4",
                "and32 r3, r4",
                "and64 r3, r4",
                "lsh r3, r4",
                "lsh32 r3, r4",
                "lsh64 r3, r4",
                "rsh r3, r4",
                "rsh32 r3, r4",
                "rsh64 r3, r4",
                "mod r3, r4",
                "mod32 r3, r4",
                "mod64 r3, r4",
                "xor r3, r4",
                "xor32 r3, r4",
                "xor64 r3, r4",
                "mov r3, r4",
                "mov32 r3, r4",
                "mov64 r3, r4",
                "arsh r3, r4",
                "arsh32 r3, r4",
                "arsh64 r3, r4",
                "hor r3, r4",
                "hor32 r3, r4",
                "hor64 r3, r4",
                "lmul r3, r4",
                "lmul32 r3, r4",
                "lmul64 r3, r4",
                "uhmul r3, r4",
                "uhmul64 r3, r4",
                "shmul r3, r4",
                "shmul64 r3, r4",
                "udiv r3, r4",
                "udiv32 r3, r4",
                "udiv64 r3, r4",
                "urem r3, r4",
                "urem32 r3, r4",
                "urem64 r3, r4",
                "sdiv r3, r4",
                "sdiv32 r3, r4",
                "sdiv64 r3, r4",
                "srem r3, r4",
                "srem32 r3, r4",
                "srem64 r3, r4",
                "ldxb r2, [r1+0]",
                "ldxh r2, [r1+0]",
                "ldxw r2, [r1+0]",
                "ldxdw r2, [r1+0]",
                "stb [r10-8], 1",
                "sth [r10-8], 1",
                "stw [r10-8], 1",
                "stdw [r10-8], 1",
                "stxb [r10-8], r1",
                "stxh [r10-8], r1",
                "stxw [r10-8], r1",
                "stxdw [r10-8], r1",
                "jeq r1, r2, +1",
                "jeq32 r1, r2, +1",
                "jeq64 r1, r2, +1",
                "jgt r1, r2, +1",
                "jgt32 r1, r2, +1",
                "jgt64 r1, r2, +1",
                "jge r1, r2, +1",
                "jge32 r1, r2, +1",
                "jge64 r1, r2, +1",
                "jlt r1, r2, +1",
                "jlt32 r1, r2, +1",
                "jlt64 r1, r2, +1",
                "jle r1, r2, +1",
                "jle32 r1, r2, +1",
                "jle64 r1, r2, +1",
                "jset r1, r2, +1",
                "jset32 r1, r2, +1",
                "jset64 r1, r2, +1",
                "jne r1, r2, +1",
                "jne32 r1, r2, +1",
                "jne64 r1, r2, +1",
                "jsgt r1, r2, +1",
                "jsgt32 r1, r2, +1",
                "jsgt64 r1, r2, +1",
                "jsge r1, r2, +1",
                "jsge32 r1, r2, +1",
                "jsge64 r1, r2, +1",
                "jslt r1, r2, +1",
                "jslt32 r1, r2, +1",
                "jslt64 r1, r2, +1",
                "jsle r1, r2, +1",
                "jsle32 r1, r2, +1",
                "jsle64 r1, r2, +1",
                "be16 r2",
                "be32 r2",
                "be64 r2",
                "le16 r2",
                "le32 r2",
                "le64 r2",
            ]
        );
    }
}
