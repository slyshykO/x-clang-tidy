# x-clang-tidy

**x-clang-tidy** is a Rust tool that makes it easy to use `clang-tidy` for static analysis of cross-compiled (embedded) C/C++ projects.
It reads configuration from a JSON file, auto-detects required include paths from your cross-compiler, and helps filter out problematic arguments—so you can seamlessly integrate `clang-tidy` into CMake or your own scripts, especially for embedded/MCU work.

---

## Features

* **Automatic GCC & Clang include detection:**
  Finds all system and C++ standard library includes used by your cross-compiler (e.g., `arm-none-eabi-gcc` or `arm-none-eabi-g++`).
* **Configurable extra arguments:**
  Easily specify target triples and custom checks for `clang-tidy`.
* **Argument filtering:**
  Use `filter-args` to remove problematic arguments (like those not understood by `clang-tidy`).
* **Supports both C and C++:**
  Language is auto-detected by the compiler name.
* **Easy integration:**
  Works smoothly as a `CMAKE_C_CLANG_TIDY`/`CMAKE_CXX_CLANG_TIDY` command in CMake, or standalone. 
* **Template support:**
  Configuration files support Handlebars templating with environment variable substitution.

---

## Template Support with Handlebars

**x-clang-tidy** supports Handlebars templating in configuration files, allowing you to use environment variables and make your configurations more flexible and portable across different development environments.

### **Using Environment Variables**

You can reference environment variables in your configuration using the `{{env "VARIABLE_NAME"}}` helper:

```json
{
  "clang-tidy": "{{env "LLVM_ROOT"}}/bin/clang-tidy.exe",
  "extra-args": [
    "--target={{env "TARGET_TRIPLE"}}",
    "-Wno-unknown-argument"
  ],
  "filter-args": [
    "-specs=nano.specs",
    "-specs=nosys.specs"
  ]
}
```

### **Template File Extension**

Configuration files with Handlebars templates should use the `.json.hbt` extension to clearly indicate they contain template syntax:

```sh
x-clang-tidy C:/gcc-arm-none-eabi/bin/arm-none-eabi-g++.exe x-clang-tidy.json.hbt src/main.cpp
```

### **Environment Variable Examples**

Set up your environment variables:

```sh
# Windows
set LLVM_ROOT=C:/LLVM
set TARGET_TRIPLE=arm-none-eabi

# Linux/macOS
export LLVM_ROOT=/usr/local/llvm
export TARGET_TRIPLE=arm-none-eabi
```

Then use a template configuration:

```json
{
  "clang-tidy": "{{env "LLVM_ROOT"}}/bin/clang-tidy",
  "extra-args": [
    "--target={{env "TARGET_TRIPLE"}}",
    "-Wno-unknown-argument"
  ],
  "filter-args": [
    "-mcpu={{env "TARGET_CPU"}}"
  ]
}
```

This approach makes your configuration portable across different machines and CI/CD environments without hardcoding paths or target-specific values.

---

## Example `x-clang-tidy.json` Configuration

```json
{
  "clang-tidy": "C:/LLVM/bin/clang-tidy.exe",
  "extra-args": [
    "--target=arm-none-eabi",
    "-Wno-unknown-argument"
  ],
  "filter-args": [
    "-specs=nano.specs",
    "-specs=nosys.specs",
    "-u _printf_float", 
    "-finline-limit=512"
  ]
}
```

* `clang-tidy`: Path to your `clang-tidy` binary.
* `extra-args`: Extra arguments (will be passed as `-extra-arg=...`).
* `filter-args`: Arguments (or argument prefixes) to **remove** from the command line, e.g., toolchain or CPU-specific flags that may break `clang-tidy`.

---

## Usage

### **Command Line**

```sh
x-clang-tidy <path-to-arm-gcc.exe or g++.exe> <path-to-x-clang-tidy.json> <clang-tidy-args...>
```

* First argument: Path to your GCC (or G++) cross-compiler.
* Second (optional) argument: path to config file. If second argument is not path to `x-clang-tidy.json` it counts as clang-tidy extra arg
* Subsequent arguments: Any arguments you would normally pass to `clang-tidy`.

**Example:**

```sh
x-clang-tidy C:/gcc-arm-none-eabi/bin/arm-none-eabi-g++.exe src/main.cpp
```

### **Custom Config File Path**

You can pass an alternative config file (e.g. for per-project settings) as an extra argument:

```sh
x-clang-tidy C:/gcc-arm-none-eabi/bin/arm-none-eabi-g++.exe D:/Projects/you-project/x-clang-tidy.json src/main.cpp
```

---

### **CMake Integration**

Add to your `CMakeLists.txt`:

```cmake
set(CMAKE_C_CLANG_TIDY "C:/path/to/x-clang-tidy.exe;C:/gcc-arm-none-eabi/bin/arm-none-eabi-gcc.exe;${CMAKE_SOURCE_DIR}/x-clang-tidy.json")
set(CMAKE_CXX_CLANG_TIDY "C:/path/to/x-clang-tidy.exe;C:/gcc-arm-none-eabi/bin/arm-none-eabi-g++.exe;${CMAKE_SOURCE_DIR}/x-clang-tidy.json")
```

* CMake will run `x-clang-tidy` with all correct options during the build.

---

## How It Works

1. **Reads config:**
   Loads `x-clang-tidy.json` `x-clang-tidy.json.hbt`.
2. **Detects language:**
   Checks if the compiler name contains `g++`/`c++` to determine C vs C++ mode.
3. **Extracts includes:**
   Runs your cross-compiler with appropriate options (`-xc` or `-xc++`) to list system and standard library include paths.
4. **Builds clang-tidy command:**

   * Adds `extra-args` from config.
   * Adds all `-I` include paths found.
   * Filters out any unwanted arguments matching `filter-args`.
   * Forwards all other arguments to `clang-tidy`.

---

## Building

You need [Rust](https://rustup.rs/) installed.

```sh
cargo build --release
```

The binary will be at `target/release/x-clang-tidy.exe`.

---

## Troubleshooting

* If headers like `cstddef` or `errno.h` aren’t found, ensure you are passing your G++ (not GCC) cross-compiler for C++ code, and that your toolchain’s include folders exist and are readable.
* Update your `filter-args` list if you see unknown argument errors from `clang-tidy`.

---

## License

MIT

---

## Contributions

PRs and feedback are welcome!

---

**Happy cross-platform static analysis!**
