#[cfg(not(target_os = "macos"))]
#[test]
fn cli_tests() {
  trycmd::TestCases::new().case("tests/README.md");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn cli_test_env_path() {
  trycmd::TestCases::new().case("tests/env-path.md");
}

#[cfg(target_os = "macos")]
#[test]
fn cli_test_env_path_macos() {
  trycmd::TestCases::new().case("tests/env-path-macos.md");
}
