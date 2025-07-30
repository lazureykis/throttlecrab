fn main() {
    // Compile protobuf files for gRPC support
    compile_protos();
}

fn compile_protos() {
    match tonic_prost_build::compile_protos("proto/throttlecrab.proto") {
        Ok(_) => println!("cargo:info=Successfully compiled protobuf"),
        Err(e) => {
            println!("cargo:warning=Failed to compile protobuf: {e}");
            println!("cargo:warning=Make sure protoc is installed:");
            println!("cargo:warning=  macOS: brew install protobuf");
            println!("cargo:warning=  Ubuntu: apt-get install protobuf-compiler");
            println!(
                "cargo:warning=  Or download from: https://github.com/protocolbuffers/protobuf/releases"
            );

            // Don't fail the build, just skip gRPC support
            std::fs::write(
                std::env::var("OUT_DIR").unwrap() + "/throttlecrab.rs",
                "// Protobuf compilation failed, gRPC support disabled\n",
            )
            .ok();
        }
    }
}