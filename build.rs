fn main() {
    // Always try to compile protos if tonic is available (for bin feature or benchmarks)
    compile_protos();
}

fn compile_protos() {
        // Set PROTOC to common homebrew path if not set
        if std::env::var("PROTOC").is_err() {
            // Try common protoc locations
            let possible_paths = [
                "/opt/homebrew/bin/protoc",  // Apple Silicon homebrew
                "/usr/local/bin/protoc",      // Intel Mac homebrew
                "/usr/bin/protoc",            // System install
            ];
            
            for path in &possible_paths {
                if std::path::Path::new(path).exists() {
                    unsafe {
                        std::env::set_var("PROTOC", path);
                    }
                    break;
                }
            }
        }
        
        match tonic_build::compile_protos("proto/throttlecrab.proto") {
            Ok(_) => println!("cargo:info=Successfully compiled protobuf"),
            Err(e) => {
                println!("cargo:warning=Failed to compile protobuf: {}", e);
                println!("cargo:warning=Make sure protoc is installed:");
                println!("cargo:warning=  macOS: brew install protobuf");
                println!("cargo:warning=  Ubuntu: apt-get install protobuf-compiler");
                println!(
                    "cargo:warning=  Or download from: https://github.com/protocolbuffers/protobuf/releases"
                );
                
                // Don't fail the build, just skip gRPC support
                std::fs::write(
                    std::env::var("OUT_DIR").unwrap() + "/throttlecrab.rs",
                    "// Protobuf compilation failed, gRPC support disabled\n"
                ).ok();
            }
        }
}
