#[path = "build/ffmpeg.rs"]
mod ffmpeg;
#[path = "build/onnxruntime.rs"]
mod onnxruntime;
fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    ffmpeg::ensure_ffmpeg_binary();
    // Silero VAD still requires ONNX Runtime, independently of ASR.
    onnxruntime::ensure_onnxruntime_runtime();
    tauri_build::build()
}
