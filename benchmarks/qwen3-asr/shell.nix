{ pkgs ? import <nixpkgs> {}, blas64 ? false }:
# qwen-asr Rust FFI declares cblas_sgemm dimensions as i32, i.e. LP64.
# nixpkgs x86_64 defaults to ILP64: never use that with this Rust binding.
let blas = pkgs.openblas.override { inherit blas64; }; in
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc rustfmt gcc gnumake cmake pkg-config blas
    python312 uv nodejs ffmpeg time git openssl ];
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib blas pkgs.zlib ];
  # VAD worker uses a declared local ORT library, never ORT build-time downloads.
  ORT_LIB_LOCATION = "${pkgs.onnxruntime}/lib";
  ORT_DYLIB_PATH = "${pkgs.onnxruntime}/lib/libonnxruntime.so";
  ORT_SKIP_DOWNLOAD = "1";
  OPENBLAS_NUM_THREADS = "6";
  OMP_NUM_THREADS = "6";
  MKL_NUM_THREADS = "6";
  HF_HUB_DISABLE_TELEMETRY = "1";
  DO_NOT_TRACK = "1";
}
