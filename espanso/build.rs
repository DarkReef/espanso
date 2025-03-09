use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
  let commit_hash = get_commit_hash();
  let out_dir = std::env::var("OUT_DIR").expect("unable to get `OUT_DIR` environment variable");

  let dest_path = Path::new(&out_dir).join("git_commit.rs");
  let mut f = File::create(&dest_path).expect("unable to create file");
  writeln!(f, "pub const GIT_COMMIT_HASH: &str = \"{commit_hash}\";").unwrap();
}

fn get_commit_hash() -> String {
  let output = Command::new("git").args(["rev-parse", "HEAD"]).output();

  match output {
    Ok(output) => {
      if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
      } else {
        "COMMIT UNKNOWN".to_string()
      }
    }
    Err(_) => "Unable to get the commit".to_string(),
  }
}
