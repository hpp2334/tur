use boa_engine::Source;

#[test]
fn bug_original() {
    let mut ctx = boa_engine::Context::default();
    let r = ctx.eval(Source::from_bytes(
        r#"function g(x){var c=0;try{if(x)return -1;c=1}catch(e){c=2}finally{c=3}return 42}g(0)"#,
    )).unwrap();
    assert_eq!(r.as_number().unwrap_or(f64::NAN), 42.0, "original with finally: {:?}", r);
}

#[test]
fn workaround_duplicate_finally_into_try_catch() {
    let mut ctx = boa_engine::Context::default();
    let r = ctx.eval(Source::from_bytes(
        r#"function g(x){var c=0;try{if(x)return -1;c=1;c=3}catch(e){c=2;c=3}return 42}g(0)"#,
    )).unwrap();
    assert_eq!(r.as_number().unwrap_or(f64::NAN), 42.0, "duplicated finally stmts: {:?}", r);
}
