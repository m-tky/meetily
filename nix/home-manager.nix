{ config, lib, pkgs, ... }:
let
  cfg = config.programs.meetily;
  worker = pkgs.callPackage ./worker.nix { };
  defaultPackage = pkgs.callPackage ./package.nix { qwenWorker = worker; };
  privateProfile = cfg.profileDirectory != null;
  profile = if privateProfile then cfg.profileDirectory else "";
  alsaConfig = pkgs.writeText "meetily-alsa.conf" ''
    defaults.namehint.showall off
    defaults.namehint.basic on
    defaults.namehint.extended off
    defaults.ctl.card 0
    ctl.hw { @args [ CARD ] @args.CARD { type string default 0 } type hw card $CARD }
    pcm_type.pulse { lib "${pkgs.alsa-plugins}/lib/alsa-lib/libasound_module_pcm_pulse.so" }
    pcm.!default { type pulse device "${cfg.microphoneSource}" hint { show on description "${cfg.microphoneSource} only" ioid "Input" } }
    pcm.meetily_mic { type pulse device "${cfg.microphoneSource}" hint { show on description "${cfg.microphoneSource} only" ioid "Input" } }
    ctl.!default { type hw card 999 }
  '';
  launcher = pkgs.writeShellApplication {
    name = "meetily";
    runtimeInputs = [ pkgs.coreutils pkgs.util-linux pkgs.libnotify pkgs.xdg-utils ];
    text = ''
      if [[ "''${1-}" == --help || "''${1-}" == --version ]]; then
        exec ${cfg.package}/bin/meetily "$@"
      fi
      ${lib.optionalString privateProfile ''
        root=${lib.escapeShellArg profile}
        if ! flock -n "$root/.preview.lock" true; then
          notify-send "Meetily" "Exit the test preview before starting the installed app." || true
          exit 73
        fi
        export HOME="$root/home"
        export XDG_DATA_HOME="$root/data"
        export XDG_CONFIG_HOME="$root/config"
        export XDG_CACHE_HOME="$root/cache"
      ''}
      export PULSE_SERVER="''${PULSE_SERVER:-unix:/run/user/$(id -u)/pulse/native}"
      export ALSA_CONFIG_PATH=${alsaConfig}
      export GDK_BACKEND=x11
      export MEETILY_DISABLE_ANALYTICS=1
      exec ${cfg.package}/bin/meetily "$@"
    '';
  };
  qwenConfig = if privateProfile then "${profile}/config/com.meetily.ai/qwen-asr.json" else "${config.xdg.configHome}/com.meetily.ai/qwen-asr.json";
in {
  options.programs.meetily = {
    enable = lib.mkEnableOption "Meetily with source-built local Qwen and ChatGPT plan support";
    package = lib.mkOption { type = lib.types.package; default = defaultPackage; };
    # Strings deliberately: credentials/models must never be copied into the Nix store.
    profileDirectory = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; description = "Reuse an existing isolated app profile without moving credentials or recordings."; };
    qwenModelDirectory = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; description = "Writable prepared official Qwen3-ASR 1.7B directory, outside the Nix store."; };
    microphoneSource = lib.mkOption { type = lib.types.str; default = "EchoCancel_Mic"; };
  };
  config = lib.mkIf cfg.enable {
    assertions = [ { assertion = builtins.match "[A-Za-z0-9_.-]+" cfg.microphoneSource != null; message = "Meetily microphoneSource must be a Pulse source identity."; } ];
    home.packages = [ launcher ];
    xdg.desktopEntries.meetily = {
      name = "Meetily";
      genericName = "Meeting assistant";
      icon = "${../frontend/src-tauri/icons/128x128.png}";
      exec = "${launcher}/bin/meetily";
      terminal = false;
      categories = [ "AudioVideo" "Office" ];
      settings.StartupWMClass = "meetily";
    };
    home.activation.meetilySetup = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      ${lib.optionalString privateProfile ''
        run mkdir -p ${lib.escapeShellArg profile}/{home,data,config,cache}
        run chmod 700 ${lib.escapeShellArg profile}
      ''}
      ${lib.optionalString (cfg.qwenModelDirectory != null) ''
        run ${pkgs.python3}/bin/python3 ${./update-qwen-paths.py} \
          ${lib.escapeShellArg qwenConfig} \
          ${cfg.package}/libexec/qwen-speech-worker \
          ${lib.escapeShellArg cfg.qwenModelDirectory} \
          ${lib.escapeShellArg cfg.package.ortLibrary}
      ''}
    '';
  };
}
