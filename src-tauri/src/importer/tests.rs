use super::tei::parse;

#[test]
fn test_import_lb() {
    let xml = "<root>line 1<lb/>line 2<lb n=\"5\"/>line 3</root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "line 1//line 2//5line 3");
}

#[test]
fn test_import_pb() {
    let xml = "<root>page 1<pb/>page 2<pb n=\"10v\"/>page 3</root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "page 1///page 2///10vpage 3");
}

#[test]
fn test_import_choice() {
    let xml = "<root>word <choice><abbr>a</abbr><expan>abbr</expan></choice> end</root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "word .abbr[a]{abbr} end");
}

#[test]
fn test_import_gap() {
    let xml = "<root>start <gap/> end</root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "start [...] end");
}

#[test]
fn test_import_supplied() {
    let xml = "<root>start <supplied>missing</supplied> end</root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "start <missing> end");
}

#[test]
fn test_import_del_add() {
    let xml = "<root><del>deleted</del><add>added</add></root>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "-{deleted}-+{added}+");
}

#[test]
fn test_import_complex() {
    let xml = "<TEI><text><body><p>Line 1<lb/>Line 2</p></body></text></TEI>";
    let dsl = parse(xml).unwrap();
    assert_eq!(dsl, "Line 1//Line 2");
}
