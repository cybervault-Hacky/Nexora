fn main() {
    // SQLx's migrate! macro embeds existing files, but Cargo cannot otherwise
    // reliably detect additions/removals under the migration directory.
    println!("cargo:rerun-if-changed=migrations");
}
