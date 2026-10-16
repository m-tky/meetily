fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Check if blas feature is enabled via CARGO_FEATURE_BLAS env var
    if std::env::var("CARGO_FEATURE_BLAS").is_ok() {
        match target_os.as_str() {
            "macos" => {
                println!("cargo:rustc-link-lib=framework=Accelerate");
            }
            "linux" => {
                println!("cargo:rustc-link-lib=openblas");
            }
            "windows" => {
                println!("cargo:rerun-if-env-changed=QWEN_OPENBLAS_LIB_DIR");
                println!("cargo:rerun-if-env-changed=QWEN_OPENBLAS_LIB_NAME");
                let dir = std::env::var("QWEN_OPENBLAS_LIB_DIR").expect(
                    "Windows BLAS requires QWEN_OPENBLAS_LIB_DIR (MSVC LP64 import library)",
                );
                assert!(
                    std::path::Path::new(&dir).is_absolute() && std::path::Path::new(&dir).is_dir(),
                    "QWEN_OPENBLAS_LIB_DIR must be an existing absolute directory"
                );
                println!("cargo:rustc-link-search=native={dir}");
                let name = std::env::var("QWEN_OPENBLAS_LIB_NAME")
                    .unwrap_or_else(|_| "libopenblas".into());
                println!("cargo:rustc-link-lib=dylib={name}");
            }
            _ => {
                // No BLAS available, will use fallback matmul
            }
        }
    }
}
