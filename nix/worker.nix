{ lib, rustPlatform, fetchFromGitHub, pkg-config, openssl, openblas, onnxruntime, cpuTarget ? "znver3" }:
let
  blas = openblas.override { blas64 = false; };
  # Same pinned engine, now retained in the checkout with its Windows patches.
  engine = ../vendor/qwen-asr;
in rustPlatform.buildRustPackage {
  pname = "qwen-speech-worker";
  version = "0.1.0";
  src = ../benchmarks/qwen3-asr/worker;
  cargoLock = { lockFile = ../benchmarks/qwen3-asr/worker/Cargo.lock; allowBuiltinFetchGit = true; };
  postPatch = ''
    substituteInPlace Cargo.toml --replace-fail \
      '../../../vendor/qwen-asr' '${engine}'
  '';
  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl blas ];
  env = {
    RUSTFLAGS = "-C target-cpu=${cpuTarget}";
    OPENBLAS_LIB_DIR = "${blas}/lib";
    OPENBLAS_INCLUDE_DIR = "${blas}/include";
    ORT_LIB_LOCATION = "${onnxruntime}/lib";
    ORT_PREFER_DYNAMIC_LINK = "1";
  };
  doCheck = true;
  cargoTestFlags = [ "--lib" ];
  meta = { mainProgram = "qwen-speech-worker"; platforms = [ "x86_64-linux" ]; license = lib.licenses.mit; };
}
