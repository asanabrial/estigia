//! The `estigia` binary. Everything it does lives in the library beside it,
//! so the tests reach it without going through a process.

use std::process::ExitCode;

fn main() -> ExitCode {
    estigia::cli::run()
}
