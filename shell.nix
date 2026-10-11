{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc rustfmt nodejs pnpm_10 cmake pkg-config clang llvmPackages.libclang
    gtk3 webkitgtk_4_1 libsoup_3 glib openssl alsa-lib libayatana-appindicator
    ffmpeg pulseaudio alsa-utils xdotool ];
  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [ stdenv.cc.cc.lib openssl alsa-lib libayatana-appindicator ]);
}
