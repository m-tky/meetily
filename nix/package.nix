{ lib, stdenv, rustPlatform, pnpm_10, fetchPnpmDeps, pnpmConfigHook, nodejs,
  pkg-config, cmake, clang, llvmPackages, python3, makeWrapper, wrapGAppsHook3,
  gtk3, webkitgtk_4_1, libsoup_3, openssl, alsa-lib, pulseaudio, libayatana-appindicator,
  onnxruntime, ffmpeg_7, source-sans, qwenWorker }:
let
  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let relative = lib.removePrefix (toString ../. + "/") (toString path);
          top = builtins.head (lib.splitString "/" relative);
          name = baseNameOf path;
      in builtins.elem top [ "Cargo.toml" "Cargo.lock" "frontend" "llama-helper" ]
        && relative != "frontend/tests"
        && !(lib.hasPrefix ".env" name)
        && !(builtins.elem name [ ".git" "node_modules" "target" ".next" "out" "binaries" "__pycache__" "official-siwc" ]);
  };
  target = stdenv.hostPlatform.rust.rustcTarget;
in rustPlatform.buildRustPackage {
  pname = "meetily-local";
  version = "0.4.1-local";
  inherit src;
  cargoLock = { lockFile = ../Cargo.lock; allowBuiltinFetchGit = true; };
  pnpmDeps = fetchPnpmDeps {
    pname = "meetily-frontend"; version = "0.4.1";
    src = lib.fileset.toSource {
      root = ../frontend;
      fileset = lib.fileset.unions [ ../frontend/package.json ../frontend/pnpm-lock.yaml ];
    };
    pnpm = pnpm_10;
    fetcherVersion = 3;
    hash = "sha256-DSN+2S3sUKyBWf8ytndyeU+XksXvkrNSXCIdVD/wg4E=";
  };
  pnpmRoot = "frontend";
  nativeBuildInputs = [ pkg-config cmake clang llvmPackages.libclang python3 makeWrapper wrapGAppsHook3 nodejs pnpm_10 pnpmConfigHook ];
  buildInputs = [ gtk3 webkitgtk_4_1 libsoup_3 openssl alsa-lib pulseaudio libayatana-appindicator onnxruntime ];
  buildType = "debug";
  cargoBuildFlags = [ "-p" "meetily" "-p" "llama-helper" ];
  buildFeatures = [ "tauri/custom-protocol" ];
  env = {
    LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
    ORT_LIB_LOCATION = "${onnxruntime}/lib";
    ORT_PREFER_DYNAMIC_LINK = "1";
    NEXT_TELEMETRY_DISABLED = "1";
    MEETILY_DISABLE_ANALYTICS = "1";
    TAURI_CONFIG = builtins.toJSON {
      build = { beforeBuildCommand = ""; devUrl = null; };
      bundle.createUpdaterArtifacts = false;
      plugins.updater.endpoints = [ ];
    };
  };
  postPatch = ''
    # No build-time network: use a pinned, packaged copy of the same font family.
    for font in ${source-sans}/share/fonts/{variable,opentype}/SourceSans3VF-Upright.otf; do
      if test -f "$font"; then cp "$font" frontend/src/app/source-sans.otf; break; fi
    done
    test -s frontend/src/app/source-sans.otf
    substituteInPlace frontend/next.config.js --replace-fail 'const nextConfig = {' \
      "const nextConfig = { generateBuildId: async () => '${builtins.hashString "sha256" "${src}:${source-sans}:${nodejs}"}',"
    substituteInPlace frontend/src/app/layout.tsx \
      --replace-fail "import { Source_Sans_3 } from 'next/font/google'" "import localFont from 'next/font/local'" \
      --replace-fail 'const sourceSans3 = Source_Sans_3({' 'const sourceSans3 = localFont({' \
      --replace-fail "subsets: ['latin']," "src: './source-sans.otf'," \
      --replace-fail "weight: ['400', '500', '600', '700']," "weight: '400 700',"
    mkdir -p frontend/src-tauri/binaries
    cp ${ffmpeg_7}/bin/ffmpeg frontend/src-tauri/binaries/ffmpeg-${target}
    # Tauri checks sidecar names at build time; the real helper is installed below.
    cp ${stdenv.shell} frontend/src-tauri/binaries/llama-helper-${target}
  '';
  preBuild = ''
    (cd frontend; pnpm build)
  '';
  doCheck = false; # UI/audio tests run separately; no capture or cloud in a package build.
  dontCargoInstall = true;
  installPhase = ''
    runHook preInstall
    mkdir -p $out/libexec $out/bin $out/lib/meetily
    cp target/${target}/debug/meetily target/${target}/debug/llama-helper $out/libexec/
    ln -s ${ffmpeg_7}/bin/ffmpeg $out/libexec/ffmpeg
    ln -s ${qwenWorker}/bin/qwen-speech-worker $out/libexec/qwen-speech-worker
    cp -r frontend/src-tauri/templates $out/libexec/templates
    cp -r frontend/src-tauri/templates $out/lib/meetily/templates
    makeWrapper $out/libexec/meetily $out/libexec/meetily-launch \
      --prefix LD_LIBRARY_PATH : '${lib.makeLibraryPath [ onnxruntime pulseaudio libayatana-appindicator ]}' \
      --set MEETILY_DISABLE_ANALYTICS 1
    printf '%s\n' '#!${stdenv.shell}' \
      'case "''${1-}" in --help|--version) printf "%s\\n" "Meetily 0.4.1-local (GUI; launch without flags)"; exit 0;; esac' \
      "exec $out/libexec/meetily-launch \"\$@\"" > $out/bin/meetily
    chmod +x $out/bin/meetily
    runHook postInstall
  '';
  passthru = { inherit qwenWorker; ortLibrary = "${onnxruntime}/lib/libonnxruntime.so"; };
  meta = { mainProgram = "meetily"; platforms = [ "x86_64-linux" ]; license = lib.licenses.mit; };
}
