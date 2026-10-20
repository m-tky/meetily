{ lib, callPackage, qwenWorker, writeShellApplication, python3, cacert }:
let
  initializer = callPackage ./model-initializer.nix { inherit qwenWorker; };
in writeShellApplication {
  name = "prepare-qwen-model";
  runtimeInputs = [ python3 ];
  text = ''
    export SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt
    exec python3 ${./prepare-qwen-model.py} \
      --manifest ${./qwen-model.json} \
      --initializer ${initializer}/bin/qwen-model-init "$@"
  '';
}
