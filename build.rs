use git_version::git_version;

// git describe --tags --dirty --always
pub const GIT_VERSION: &str = git_version!(args = ["--tags", "--dirty", "--always"]);

fn main() {
    println!("cargo:rustc-env=_GIT_VERSION={GIT_VERSION}");
}
