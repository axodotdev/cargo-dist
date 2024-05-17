#[test]
fn test_trybuild() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/empty_main.rs");
    t.compile_fail("tests/trybuild/fail_to_compile.rs");
}
