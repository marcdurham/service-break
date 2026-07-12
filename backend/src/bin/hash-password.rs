//! Prints the Argon2 PHC hash of a password read from stdin — the exact
//! format the backend stores in `users.password_hash`.
//!
//! Used by `scripts/change-password.sh` to reset passwords directly in the
//! database; not part of the running server. The password arrives on stdin
//! (not argv) so it never shows up in `ps` or shell history:
//!
//! ```sh
//! printf '%s' 'new password' | cargo run -q -p backend --bin hash-password
//! ```

use std::io::Read;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("could not read the password from stdin: {e}");
        return ExitCode::FAILURE;
    }
    // Trailing newlines come from `echo` / heredoc plumbing, not the
    // password; anything else (including spaces) is kept verbatim.
    let password = input.trim_end_matches(['\r', '\n']);
    if let Err(e) = shared::validate_password(password) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    match backend::auth::hash_password(password) {
        Ok(hash) => {
            println!("{hash}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
