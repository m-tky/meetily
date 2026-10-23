{ lib, callPackage, qwenWorker, writeShellApplication, python3, cacert, modelSize ? "1.7b" }:
assert builtins.elem modelSize [ "1.7b" "0.6b" ];
let
  initializer = callPackage ./model-initializer.nix { inherit qwenWorker; };
  manifest = if modelSize == "0.6b" then ./qwen-model-0.6b.json else ./qwen-model.json;
in writeShellApplication {
  name = "prepare-qwen-model";
  runtimeInputs = [ python3 ];
  text = ''
    export SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt
    exec python3 ${./prepare-qwen-model.py} \
      --manifest ${manifest} \
      --initializer ${initializer}/bin/qwen-model-init "$@"
  '';
}
