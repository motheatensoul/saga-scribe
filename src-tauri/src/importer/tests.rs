use super::tei::parse;

#[test]
fn test_import_lb() {
    let xml = "<body>line 1<lb/>line 2<lb n=\"5\"/>line 3</body>";
    let dsl = parse(xml).unwrap();
    // lb adds a newline BEFORE the marker
    assert_eq!(dsl, "line 1\n//line 2\n//5line 3");
}

#[test]
fn test_import_pb() {
    let xml = "<body>page 1<pb/>page 2<pb n=\"10v\"/>page 3</body>";
    let dsl = parse(xml).unwrap();
    // pb adds a newline BEFORE the marker
    assert_eq!(dsl, "page 1\n///page 2\n///10vpage 3");
}

#[test]
fn test_import_choice() {
    let xml = "<body>word <choice><abbr>a</abbr><expan>abbr</expan></choice> end</body>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "word .abbr[a]{abbr} end");
}

#[test]
fn test_import_gap() {
    let xml = "<body>start <gap/> end</body>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "start [...] end");
}

#[test]
fn test_import_supplied() {
    let xml = "<body>start <supplied>missing</supplied> end</body>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "start <missing> end");
}

#[test]
fn test_import_del_add() {
    let xml = "<body><del>deleted</del><add>added</add></body>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "-{deleted}-+{added}+");
}

#[test]
fn test_import_complex() {
    let xml = "<TEI><text><body><p>Line 1<lb/>Line 2</p></body></text></TEI>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "Line 1\n//Line 2");
}
