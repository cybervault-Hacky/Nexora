pub mod analysis;
pub mod commands;
pub mod patch;
// No command reaches the patch contract yet. Building it only for tests keeps
// its contract types and its test suite alive without shipping dead code in
// the application binary.
#[cfg(test)]
pub mod patch_contract;
pub mod scanner;
pub mod settings;
pub mod symbol_index;
pub mod validation;
