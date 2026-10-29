//! Differential tests: the syntax-tree parser must reproduce the original
//! lexer exactly. Wherever the old lexer accepted input, lowering the tree
//! gives the same nodes; wherever it rejected input, the new parser rejects it
//! with the same message. The only intended difference: errors inside block
//! content (`.head{...}` etc.) are now reported, where the old lexer accepted
//! the block and its content failed later, when compiled.

use super::legacy_lexer::LegacyLexer;
use super::syntax;

fn assert_equivalent(source: &str) {
    let legacy = LegacyLexer::new(source).parse();
    let tree = syntax::parse(source);
    match legacy {
        Ok(doc) => {
            assert!(
                tree.diagnostics.iter().all(|d| d.nested),
                "old lexer accepted {:?} but new parser reports {:?}",
                source,
                tree.diagnostics
            );
            assert_eq!(
                syntax::lower(&tree.nodes, source),
                doc.nodes,
                "different nodes for {:?}",
                source
            );
        }
        Err(message) => {
            let first = tree.diagnostics.iter().find(|d| !d.nested);
            assert_eq!(
                first.map(|d| d.message.as_str()),
                Some(message.as_str()),
                "old lexer rejected {:?}; new parser reports {:?}",
                source,
                tree.diagnostics
            );
        }
    }
}

#[test]
fn test_matches_legacy_on_known_cases() {
    let cases = [
        "",
        "plain text",
        "//1 maþr //2 ok ///1r ///1v",
        "//2Text //x ///",
        "~//5abc ~///12v word~//",
        "upp~haf a|b ~ |",
        ".abbr[k]{konungr} .abbr[a[b]c]{d} .abbr[x] .abbr[x]{",
        "[...] [...3] [...<txt>] [...12<t x>] [...99999999999] [...<x] [...3",
        "<sup> <<not> <unclosed",
        "-{del}- -{a<b}- -{x} -{y",
        "+{add}+ +{x}- ?{u}? ?{u} ^{note} ^{open",
        ":et: :not an entity: ::  :a_1: :þ:",
        ".head{//1 Upphaf :et:} .head{a{b}c} .head{open",
        ".supplied{x -{y}- z} .norm{a} .norm{",
        ".head{a -{b}} trailing",
        "mixed þ ð æ ǫ ꝛ \u{E8B7} //3 end",
        ".p start .p\nmid .p end .p",
        "word.p .prolog .p. a.p b .head{.p} //4 .p //5",
    ];
    for case in cases {
        assert_equivalent(case);
    }
}

#[test]
fn test_matches_legacy_on_menota_document() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/HolmPerg-34-4to-MLL.xml");
    let Ok(xml) = std::fs::read_to_string(path) else { return };
    let dsl = crate::importer::tei::parse(&xml).unwrap().dsl;
    assert_equivalent(&dsl);
    // And every line on its own, which exercises many more boundaries
    for line in dsl.lines() {
        assert_equivalent(line);
    }
}

#[test]
fn test_matches_legacy_on_fuzzed_input() {
    const PIECES: &[&str] = &[
        "//", "///", "~", "|", ":", "et", "a", "þ", " ", "\n", "1", "3v", "-", "+", "?", "^",
        "{", "}", "[", "]", "<", ">", "-{", "}-", "+{", "}+", "?{", "}?", "^{", "[...", "<<",
        ".abbr[", ".head{", ".supplied{", ".norm{", ".", "~//", "~///", ":et:", "ꝛ",
        ".p", "p",
    ];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = |n: usize| {
        // xorshift64*: high bits, so small moduli are well distributed
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        ((state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize) % n
    };
    for _ in 0..50_000 {
        let len = 1 + next(24);
        let source: String = (0..len).map(|_| PIECES[next(PIECES.len())]).collect();
        assert_equivalent(&source);
    }
}
