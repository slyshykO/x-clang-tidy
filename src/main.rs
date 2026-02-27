use handlebars::{Handlebars, handlebars_helper};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::sync::OnceLock;

static CWD: OnceLock<std::path::PathBuf> = OnceLock::new();
const __VERSION: &str = concat!("\0", "Ver.:", env!("_GIT_VERSION"), "\0");
const _VERSION: &str = env!("_GIT_VERSION");

#[derive(Deserialize)]
struct Config {
    #[serde(rename = "clang-tidy")]
    clang_tidy: String,
    #[serde(rename = "extra-args")]
    extra_args: Vec<String>,
    #[serde(rename = "filter-args")]
    filter_args: Option<Vec<String>>,
    #[serde(rename = "filter-files")]
    filter_files: Option<Vec<String>>,

    checks: Option<Vec<String>>,
}

pub fn cwd() -> &'static std::path::PathBuf {
    CWD.get().expect("CWD not set")
}

pub(crate) fn app_version() -> String {
    __VERSION
        .trim_matches('\0')
        .strip_prefix("Ver.:")
        .unwrap_or(_VERSION)
        .to_string()
}

fn main() -> ExitCode {
    let cwd_ = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if CWD.set(cwd_).is_err() {
        eprintln!("Error: can't set CWD");
        return ExitCode::FAILURE;
    };
    match _main() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn config_text(additional_path: &[String]) -> anyhow::Result<String> {
    let results = additional_path
        .iter()
        .map(fs::read_to_string)
        .filter(|s| s.is_ok())
        .map(|s| s.unwrap_or_default())
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err(anyhow::anyhow!(
            "No valid configuration files found in provided paths"
        ))
    } else {
        Ok(results[0].clone())
    }
}

fn is_valid_config_path<T: AsRef<str>>(path: T) -> bool {
    let path = path.as_ref();
    std::path::Path::new(path).exists() && (path.ends_with(".json") || path.ends_with(".json.hbt"))
}

// a helper that return env variables
handlebars_helper!(hb_env: |name: String| {
    match std::env::var(&name) {
        Ok(value) => {
            //check if the value is existing path
            if std::path::Path::new(&value).exists() {
                value.replace("\\", "/").to_string()
            } else {
                value
            }
        },
        Err(e) => format!("{name}: {e}"),
    }
});

fn _main() -> anyhow::Result<()> {
    // Usage: x-clang-tidy <path-to-cross-compiler> <clang-tidy-args...>
    let args = std::env::args().collect::<Vec<String>>();

    eprintln!("x-clang-tidy: {}", app_version());
    eprintln!("x-clang-tidy cwd: {}", cwd().display());

    let cross_compiler_path = args
        .get(1)
        .expect("First argument should be path to cross-compiler");
    let extra_args: Vec<String> = args[2..].to_vec();

    let conf_additional_path = {
        let mut v = extra_args
            .iter()
            .filter(|arg| is_valid_config_path(arg))
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        v.insert(0, "x-clang-tidy.json".to_string());
        v.insert(0, "x-clang-tidy.json.hbt".to_string());
        v
    };

    let src_files = extra_args
        .iter()
        .take_while(|i| *i != "--")
        .filter(|i| Path::new(i).exists())
        .filter(|i| !is_valid_config_path(i))
        .map(|s| s.to_owned())
        .collect::<Vec<_>>();

    // create the handlebars registry
    let mut handlebars = Handlebars::new();
    handlebars.register_helper("env", Box::new(hb_env));

    // Read config
    let config_text = {
        let txt = config_text(&conf_additional_path)?;
        handlebars.render_template(&txt, &())?
    };
    let config: Config =
        serde_json::from_str(&config_text).expect("Failed to parse x-clang-tidy.json");

    // check if any of the source are match with at least one of the filter-files patterns
    if let Some(filter_files) = &config.filter_files {
        for src in &src_files {
            for pattern in filter_files {
                match wildcard::WildcardBuilder::new(pattern.as_bytes()).build() {
                    Ok(wc) => {
                        if wc.is_match(src.as_bytes()) {
                            println!("Source file `{src}` matches filter pattern `{pattern}`");
                            return Ok(());
                        }
                    }
                    Err(e) => eprintln!("Invalid pattern `{pattern}`: {e}"),
                }
            }
        }
    }

    let mut compiler_extra_args: Vec<String> = Vec::new();
    // find --target= argument
    if let Some(target_arg) = extra_args.iter().find(|arg| arg.starts_with("--target=")) {
        compiler_extra_args.push(target_arg.clone());
    }
    // find --config= argument
    if let Some(config_arg) = extra_args.iter().find(|arg| arg.starts_with("--config=")) {
        compiler_extra_args.push(config_arg.clone());
    }

    // Get GCC system include paths
    let include_paths = extract_compiler_includes(cross_compiler_path, &compiler_extra_args)?;

    let clang_tidy_args = match config.filter_args {
        Some(filter_args) => {
            let ea = extra_args
                .into_iter()
                .filter(|arg| !is_valid_config_path(arg))
                .collect::<Vec<_>>();

            // Create a set of args to filter out, handling both individual args and space-separated options
            let mut filtered_out = std::collections::HashSet::new();
            let mut filtered_out_patterns = Vec::new();
            for filter_arg in &filter_args {
                if filter_arg.contains(' ') {
                    // Split space-separated option and add individual parts
                    let parts: Vec<&str> = filter_arg.split_whitespace().collect();
                    for part in parts {
                        filtered_out.insert(part.to_string());
                    }
                } else if filter_arg.contains('*') {
                    filtered_out_patterns.push(filter_arg.to_string());
                } else {
                    filtered_out.insert(filter_arg.clone());
                }
            }

            // Filter out args based on exact matches and wildcard patterns
            ea.into_iter()
                .filter(|arg| {
                    filtered_out_patterns.iter().all(|pattern| {
                        match wildcard::WildcardBuilder::new(pattern.as_bytes()).build() {
                            Ok(wc) => !wc.is_match(arg.as_bytes()),
                            Err(_) => {
                                eprintln!("Invalid pattern: {}", pattern);
                                true
                            } // If pattern is invalid, skip filtering
                        }
                    })
                })
                .filter(|arg| !filtered_out.contains(arg))
                .collect::<Vec<_>>()
        }
        None => extra_args
            .into_iter()
            .filter(|arg| !is_valid_config_path(arg))
            .collect::<Vec<_>>(),
    };

    eprintln!("all args            : {args:?}");
    eprintln!("compiler path       : {cross_compiler_path}");
    eprintln!("source files        : {src_files:?}");
    eprintln!("clang-tidy args     : {clang_tidy_args:?}");
    eprintln!("conf additional path: {conf_additional_path:?}");

    let checks_arg = if let Some(checks) = &config.checks {
        if !checks.is_empty() {
            format!("--checks={}", checks.join(","))
        } else {
            String::from("--checks=*")
        }
    } else {
        String::from("--checks=*")
    };
    eprintln!("clang-tidy checks   : {checks_arg}");

    // Build clang-tidy command
    let mut cmd = Command::new(&config.clang_tidy);
    cmd.arg(checks_arg);
    for arg in &config.extra_args {
        cmd.arg(format!("-extra-arg={arg}"));
    }
    for path in include_paths {
        cmd.arg(format!("-extra-arg=-I{path}"));
    }
    for arg in &clang_tidy_args {
        cmd.arg(arg);
    }

    // Run clang-tidy
    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow::anyhow!("Can't launch `{}`: {}", &config.clang_tidy, e))
        .expect("Failed to execute clang-tidy");

    match status.code() {
        Some(code) => {
            if code == 0 {
                Ok(())
            } else {
                Err(anyhow::anyhow!("clang-tidy exited with code {}", code))
            }
        }
        None => Err(anyhow::anyhow!("clang-tidy terminated by signal")),
    }
}

fn is_cpp_compiler(compiler_path: &str) -> bool {
    let compiler_lower = compiler_path.to_ascii_lowercase();
    compiler_lower.contains("g++") || compiler_lower.contains("c++")
}

fn extract_compiler_includes(gcc: &str, extra_args: &[String]) -> anyhow::Result<Vec<String>> {
    let is_cpp = is_cpp_compiler(gcc);
    let lang_flag = if is_cpp { "-xc++" } else { "-xc" };
    // Run gcc -xc -E -v -
    let output = Command::new(gcc)
        .args(extra_args)
        .args([lang_flag, "-E", "-v", "-"])
        .stdin(Stdio::null())
        .output()
        .expect("Failed to run gcc to extract include paths");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut includes = Vec::new();
    let mut in_block = false;
    for line in stderr.lines() {
        if line
            .trim()
            .starts_with("#include <...> search starts here:")
        {
            in_block = true;
            continue;
        }
        if line.trim().starts_with("End of search list.") {
            break;
        }
        if in_block {
            let path = line.trim();
            // Only add if path exists and isn't empty
            if !path.is_empty() && std::path::Path::new(path).exists() {
                includes.push(path.replace("\\", "/")); // Normalize path
            }
        }
    }
    Ok(includes)
}
