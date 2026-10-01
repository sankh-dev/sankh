fn main() {
    // Re-embed the UI whenever the frontend build output changes.
    println!("cargo:rerun-if-changed=../../frontend/dist");
}
