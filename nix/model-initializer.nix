{ lib, qwenWorker }:
qwenWorker.overrideAttrs (old: {
  pname = "qwen-model-init";
  src = lib.fileset.toSource {
    root = ../benchmarks/qwen3-asr/worker;
    fileset = lib.fileset.unions [ ../benchmarks/qwen3-asr/worker/Cargo.toml ../benchmarks/qwen3-asr/worker/Cargo.lock ];
  };
  postPatch = old.postPatch + ''
    mkdir -p src
    cp ${./model-init.rs} src/main.rs
  '';
  doCheck = false;
  postInstall = ''
    mv $out/bin/qwen-speech-worker $out/bin/qwen-model-init
  '';
  meta = old.meta // { mainProgram = "qwen-model-init"; };
})
