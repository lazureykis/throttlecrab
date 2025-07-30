fn main() {
    // Only compile protos if the bin feature is enabled
    #[cfg(feature = "bin")]
    {
        match tonic_build::compile_protos("proto/throttlecrab.proto") {
            Ok(_) => println!("cargo:info=Successfully compiled protobuf"),
            Err(e) => {
                println!("cargo:warning=Failed to compile protobuf: {}", e);
                println!("cargo:warning=Make sure protoc is installed:");
                println!("cargo:warning=  macOS: brew install protobuf");
                println!("cargo:warning=  Ubuntu: apt-get install protobuf-compiler");
                println!("cargo:warning=  Or download from: https://github.com/protocolbuffers/protobuf/releases");
            }
        }
    }
}
