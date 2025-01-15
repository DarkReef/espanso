#[cfg(not(target_os = "macos"))]
#[test]
fn cli_tests() {
  trycmd::TestCases::new().case("tests/README.md");
}
