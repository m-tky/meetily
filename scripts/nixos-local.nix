# Local-only NixOS standalone candidate, NOT a portable .deb/AppImage or release.
# `inputs` must hold a custom-protocol build (embedded frontend, not cargo dev).
{ inputs, pkgs ? import <nixpkgs> {} }:
let
  source = builtins.path { path = inputs; name = "meetily-local-inputs"; };
  # Raw binaries copied from outside the store don't automatically retain their
  # embedded RPATH references. Keep the worker's required LP64 BLAS and ORT in
  # the explicit derivation closure as well; never substitute ILP64 OpenBLAS.
  qwenBlas = pkgs.openblas.override { blas64 = false; };
  runtime = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib pkgs.openssl
    pkgs.alsa-lib pkgs.pulseaudio pkgs.libayatana-appindicator qwenBlas pkgs.onnxruntime ];
in pkgs.runCommandLocal "meetily-local-candidate" {
  nativeBuildInputs = [ pkgs.makeWrapper ];
} ''
  mkdir -p $out/bin $out/libexec $out/lib/meetily/templates $out/share/meetily
  cp ${source}/meetily ${source}/ffmpeg ${source}/llama-helper $out/libexec/
  cp ${source}/templates/*.json $out/lib/meetily/templates/
  # Also support exe-relative resource resolution in unpackaged native builds.
  cp -r ${source}/templates $out/libexec/
  if [ -f ${source}/qwen-speech-worker ]; then
    cp ${source}/qwen-speech-worker $out/libexec/
    chmod +x $out/libexec/qwen-speech-worker
  fi
  chmod +x $out/libexec/meetily $out/libexec/ffmpeg $out/libexec/llama-helper
  if [ -f ${source}/gui_fixture_bootstrap ] && [ -f $out/libexec/qwen-speech-worker ]; then
    cp ${source}/gui_fixture_bootstrap $out/libexec/
    cp ${./preview_synthetic.py} $out/share/meetily/preview_synthetic.py
    makeWrapper ${pkgs.python3}/bin/python3 $out/bin/meetily-synthetic-preview \
      --add-flags "$out/share/meetily/preview_synthetic.py --binary $out/bin/meetily --bootstrap $out/libexec/gui_fixture_bootstrap --worker $out/libexec/qwen-speech-worker --driver ${pkgs.webkitgtk_4_1}/bin/WebKitWebDriver --espeak ${pkgs.espeak-ng}/bin/espeak-ng" \
      --prefix PATH : '${pkgs.lib.makeBinPath [pkgs.dbus pkgs.pulseaudio pkgs.bash]}'
  fi
  makeWrapper $out/libexec/meetily $out/bin/meetily \
    --prefix LD_LIBRARY_PATH : '${runtime}' \
    --prefix XDG_DATA_DIRS : '${pkgs.gsettings-desktop-schemas}/share:${pkgs.adwaita-icon-theme}/share' \
    --prefix GSETTINGS_SCHEMA_DIR : '${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}' \
    --set MEETILY_DISABLE_ANALYTICS 1
  cat > $out/share/meetily/LOCAL-CANDIDATE.txt <<'NOTICE'
  NixOS local candidate only. Embedded frontend; no Next server or nix-shell at launch.
  Debug build, not optimized release. Original upstream updater endpoints disabled.
  ASR defaults unchanged. Optional local Qwen needs explicit settings and external model/ORT.
  Models, credentials and recordings are not bundled. Existing user profiles are not migrated.
  FFmpeg/model/third-party licenses require review before redistribution.
NOTICE
''
