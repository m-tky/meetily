{
  description = "Meetily with local Qwen ASR and optional ChatGPT plan summaries";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/8edc0c72e3a38faf5434e40d1f19431125fe4b30";
  outputs = { self, nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
    worker = pkgs.callPackage ./nix/worker.nix { };
    app = pkgs.callPackage ./nix/package.nix { qwenWorker = worker; };
    prepare = pkgs.callPackage ./nix/model-preparation.nix { qwenWorker = worker; };
  in {
    packages.${system} = { default = app; meetily = app; qwen-speech-worker = worker; prepare-qwen-model = prepare; };
    apps.${system}.prepare-qwen-model = { type = "app"; program = "${prepare}/bin/prepare-qwen-model"; };
    homeManagerModules.default = import ./nix/home-manager.nix;
  };
}
