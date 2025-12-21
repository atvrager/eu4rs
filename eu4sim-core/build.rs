//! Build script for eu4sim-core
//!
//! Compiles Cap'n Proto schema to Rust code for training data serialization.

fn main() {
    // Rerun if schema changes
    println!("cargo:rerun-if-changed=../schemas/training.capnp");

    // Compile Cap'n Proto schema
    capnpc::CompilerCommand::new()
        .src_prefix("../schemas")
        .file("../schemas/training.capnp")
        .run()
        .expect("Failed to compile Cap'n Proto schema");
}
