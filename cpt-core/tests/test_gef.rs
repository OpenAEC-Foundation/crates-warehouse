mod common;

use cpt_core::gef::parse;
use common::read_fixture;

#[test]
fn parses_voorbeeld_gef() {
    let text = read_fixture("voorbeeld.gef");
    let cpt = parse(&text).expect("voorbeeld.gef should parse");
    assert!(!cpt.points.is_empty(), "expected measurement points");
    let first = &cpt.points[0];
    assert!(first.qc.is_some(), "first point qc should be set");
    assert!(first.depth >= 0.0);
}

#[test]
fn parses_cpt_pygef_gef() {
    let text = read_fixture("cpt_pygef.gef");
    let cpt = parse(&text).expect("cpt_pygef.gef should parse");
    assert!(!cpt.points.is_empty());
}

#[test]
fn parses_2600356_series() {
    for n in 1..=6 {
        let name = format!("2600356_0{}.GEF", n);
        let text = read_fixture(&name);
        let cpt = parse(&text).expect(&format!("{} should parse", name));
        assert!(!cpt.points.is_empty(), "{} should have points", name);
        // Real-world series should have RD position
        assert!(cpt.position.is_some(), "{} should have RD position", name);
    }
}

#[test]
fn computes_rf_when_missing() {
    // qc + fs given, rf must be derived = 100*fs/qc
    let gef = r#"#GEFID= 1, 0, 0
#TESTID= TEST
#XYID= 1, 100000.0, 400000.0
#ZID= 31000, 0.0
#COLUMN= 3
#COLUMNINFO= 1, m, Sondeerlengte, 1
#COLUMNINFO= 2, MPa, Conusweerstand, 2
#COLUMNINFO= 3, MPa, Wrijving, 3
#COLUMNSEPARATOR= ;
#RECORDSEPARATOR= !
#EOH=
0.02 ; 5.0 ; 0.05 !
0.04 ; 6.0 ; 0.06 !
"#;
    let cpt = parse(gef).unwrap();
    assert_eq!(cpt.points.len(), 2);
    let p = cpt.points[0];
    assert!((p.rf.unwrap() - 1.0).abs() < 1e-6, "rf should be 1.0%, got {:?}", p.rf);
}

#[test]
fn applies_void_value() {
    let gef = r#"#GEFID= 1, 0, 0
#COLUMN= 2
#COLUMNINFO= 1, m, Length, 1
#COLUMNINFO= 2, MPa, Qc, 2
#COLUMNVOID= 2, -9999
#EOH=
0.02 -9999
0.04 5.5
"#;
    let cpt = parse(gef).unwrap();
    assert_eq!(cpt.points.len(), 2);
    assert_eq!(cpt.points[0].qc, None);
    assert_eq!(cpt.points[1].qc, Some(5.5));
}
