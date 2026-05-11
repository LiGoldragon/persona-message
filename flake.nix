{
  description = "Persona message contract and shim.";

  inputs = {
    nixpkgs.url = "github:LiGoldragon/nixpkgs?ref=main";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
    }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forSystems = function: nixpkgs.lib.genAttrs systems (system: function system);

      mkContext =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          toolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
          };
          craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
          src = craneLib.cleanCargoSource ./.;
          commonArgs = {
            inherit src;
            strictDeps = true;
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          messageConstraintCheck =
            name: script:
            pkgs.runCommand name { } ''
              set -euo pipefail

              export MESSAGE_BIN=${self.packages.${system}.default}/bin/message
              ${pkgs.bash}/bin/bash ${script}

              touch "$out"
            '';
          sourceConstraintCheck =
            name: script:
            pkgs.runCommand name { } ''
              set -euo pipefail

              export PATH=${pkgs.lib.makeBinPath [ pkgs.ripgrep ]}:$PATH
              ${pkgs.bash}/bin/bash ${script} ${./.}

              touch "$out"
            '';
        in
        {
          inherit
            pkgs
            toolchain
            craneLib
            commonArgs
            cargoArtifacts
            messageConstraintCheck
            sourceConstraintCheck
            ;
        };
    in
    {
      packages = forSystems (
        system:
        let
          context = mkContext system;
        in
        {
          test-basic = context.pkgs.writeShellScriptBin "persona-message-test-basic" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-basic} "$@"
          '';
          test-actual-harness = context.pkgs.writeShellScriptBin "persona-message-test-actual-harness" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-actual-harness} "$@"
          '';
          test-actual-codex-to-claude = context.pkgs.writeShellScriptBin "persona-message-test-actual-codex-to-claude" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-actual-codex-to-claude} "$@"
          '';
          test-actual-claude-to-codex = context.pkgs.writeShellScriptBin "persona-message-test-actual-claude-to-codex" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-actual-claude-to-codex} "$@"
          '';
          setup-harnesses = context.pkgs.writeShellScriptBin "persona-message-setup-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-harnesses} "$@"
          '';
          setup-harnesses-visible = context.pkgs.writeShellScriptBin "persona-message-setup-harnesses-visible" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-harnesses-visible} "$@"
          '';
          setup-harnesses-headless = context.pkgs.writeShellScriptBin "persona-message-setup-harnesses-headless" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-harnesses-headless} "$@"
          '';
          attach-harnesses = context.pkgs.writeShellScriptBin "persona-message-attach-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.ripgrep ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            export PERSONA_MESSAGE_SCRIPT_DIR=${./scripts}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/attach-harnesses} "$@"
          '';
          test-running-harnesses = context.pkgs.writeShellScriptBin "persona-message-test-running-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.ripgrep ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            export PERSONA_MESSAGE_SCRIPT_DIR=${./scripts}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-running-harnesses} "$@"
          '';
          teardown-harnesses = context.pkgs.writeShellScriptBin "persona-message-teardown-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            export PERSONA_MESSAGE_SCRIPT_DIR=${./scripts}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-harnesses} "$@"
          '';
          view-harnesses = context.pkgs.writeShellScriptBin "persona-message-view-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            export PERSONA_MESSAGE_SCRIPT_DIR=${./scripts}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/view-harnesses} "$@"
          '';
          setup-pty-demo = context.pkgs.writeShellScriptBin "persona-message-setup-pty-demo" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-pty-demo} "$@"
          '';
          attach-pty-demo = context.pkgs.writeShellScriptBin "persona-message-attach-pty-demo" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/attach-pty-demo} "$@"
          '';
          teardown-pty-demo = context.pkgs.writeShellScriptBin "persona-message-teardown-pty-demo" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-pty-demo} "$@"
          '';
          setup-pty-harnesses = context.pkgs.writeShellScriptBin "persona-message-setup-pty-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.python3 ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-pty-harnesses} "$@"
          '';
          setup-pty-pi = context.pkgs.writeShellScriptBin "persona-message-setup-pty-pi" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/setup-pty-pi} "$@"
          '';
          test-pty-pi-message = context.pkgs.writeShellScriptBin "persona-message-test-pty-pi-message" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.ripgrep ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-pty-pi-message} "$@"
          '';
          test-pty-pi-niri-focus = context.pkgs.writeShellScriptBin "persona-message-test-pty-pi-niri-focus" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.ripgrep context.pkgs.python3 ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-pty-pi-niri-focus} "$@"
          '';
          test-pty-pi-guarded-delivery = context.pkgs.writeShellScriptBin "persona-message-test-pty-pi-guarded-delivery" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix context.pkgs.ripgrep context.pkgs.python3 ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/test-pty-pi-guarded-delivery} "$@"
          '';
          attach-pty-harnesses = context.pkgs.writeShellScriptBin "persona-message-attach-pty-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/attach-pty-harnesses} "$@"
          '';
          attach-pty-pi = context.pkgs.writeShellScriptBin "persona-message-attach-pty-pi" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/attach-pty-pi} "$@"
          '';
          teardown-pty-harnesses = context.pkgs.writeShellScriptBin "persona-message-teardown-pty-harnesses" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-pty-harnesses} "$@"
          '';
          teardown-pty-pi = context.pkgs.writeShellScriptBin "persona-message-teardown-pty-pi" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-pty-pi} "$@"
          '';
          teardown-pty-pi-message = context.pkgs.writeShellScriptBin "persona-message-teardown-pty-pi-message" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-pty-pi-message} "$@"
          '';
          teardown-pty-pi-guarded-delivery = context.pkgs.writeShellScriptBin "persona-message-teardown-pty-pi-guarded-delivery" ''
            export PATH=${context.pkgs.lib.makeBinPath [ context.toolchain context.pkgs.nix ]}:$PATH
            export PERSONA_MESSAGE_REPO=''${PERSONA_MESSAGE_REPO:-$PWD}
            exec ${context.pkgs.bash}/bin/bash ${./scripts/teardown-pty-pi-guarded-delivery} "$@"
          '';
          default = context.craneLib.buildPackage (
            context.commonArgs
            // {
              inherit (context) cargoArtifacts;
              pname = "persona-message";
              meta.mainProgram = "message";
            }
          );
        }
      );

      apps = forSystems (
        system:
        let
          packages = self.packages.${system};
        in
        {
          default = {
            type = "app";
            program = "${packages.default}/bin/message";
          };
          test-basic = {
            type = "app";
            program = "${packages.test-basic}/bin/persona-message-test-basic";
          };
          test-actual-harness = {
            type = "app";
            program = "${packages.test-actual-harness}/bin/persona-message-test-actual-harness";
          };
          test-actual-codex-to-claude = {
            type = "app";
            program = "${packages.test-actual-codex-to-claude}/bin/persona-message-test-actual-codex-to-claude";
          };
          test-actual-claude-to-codex = {
            type = "app";
            program = "${packages.test-actual-claude-to-codex}/bin/persona-message-test-actual-claude-to-codex";
          };
          setup-harnesses = {
            type = "app";
            program = "${packages.setup-harnesses}/bin/persona-message-setup-harnesses";
          };
          setup-harnesses-visible = {
            type = "app";
            program = "${packages.setup-harnesses-visible}/bin/persona-message-setup-harnesses-visible";
          };
          setup-harnesses-headless = {
            type = "app";
            program = "${packages.setup-harnesses-headless}/bin/persona-message-setup-harnesses-headless";
          };
          attach-harnesses = {
            type = "app";
            program = "${packages.attach-harnesses}/bin/persona-message-attach-harnesses";
          };
          test-running-harnesses = {
            type = "app";
            program = "${packages.test-running-harnesses}/bin/persona-message-test-running-harnesses";
          };
          teardown-harnesses = {
            type = "app";
            program = "${packages.teardown-harnesses}/bin/persona-message-teardown-harnesses";
          };
          view-harnesses = {
            type = "app";
            program = "${packages.view-harnesses}/bin/persona-message-view-harnesses";
          };
          setup-pty-demo = {
            type = "app";
            program = "${packages.setup-pty-demo}/bin/persona-message-setup-pty-demo";
          };
          attach-pty-demo = {
            type = "app";
            program = "${packages.attach-pty-demo}/bin/persona-message-attach-pty-demo";
          };
          teardown-pty-demo = {
            type = "app";
            program = "${packages.teardown-pty-demo}/bin/persona-message-teardown-pty-demo";
          };
          setup-pty-harnesses = {
            type = "app";
            program = "${packages.setup-pty-harnesses}/bin/persona-message-setup-pty-harnesses";
          };
          setup-pty-pi = {
            type = "app";
            program = "${packages.setup-pty-pi}/bin/persona-message-setup-pty-pi";
          };
          test-pty-pi-message = {
            type = "app";
            program = "${packages.test-pty-pi-message}/bin/persona-message-test-pty-pi-message";
          };
          test-pty-pi-niri-focus = {
            type = "app";
            program = "${packages.test-pty-pi-niri-focus}/bin/persona-message-test-pty-pi-niri-focus";
          };
          test-pty-pi-guarded-delivery = {
            type = "app";
            program = "${packages.test-pty-pi-guarded-delivery}/bin/persona-message-test-pty-pi-guarded-delivery";
          };
          attach-pty-harnesses = {
            type = "app";
            program = "${packages.attach-pty-harnesses}/bin/persona-message-attach-pty-harnesses";
          };
          attach-pty-pi = {
            type = "app";
            program = "${packages.attach-pty-pi}/bin/persona-message-attach-pty-pi";
          };
          teardown-pty-harnesses = {
            type = "app";
            program = "${packages.teardown-pty-harnesses}/bin/persona-message-teardown-pty-harnesses";
          };
          teardown-pty-pi = {
            type = "app";
            program = "${packages.teardown-pty-pi}/bin/persona-message-teardown-pty-pi";
          };
          teardown-pty-pi-message = {
            type = "app";
            program = "${packages.teardown-pty-pi-message}/bin/persona-message-teardown-pty-pi-message";
          };
          teardown-pty-pi-guarded-delivery = {
            type = "app";
            program = "${packages.teardown-pty-pi-guarded-delivery}/bin/persona-message-teardown-pty-pi-guarded-delivery";
          };
        }
      );

      checks = forSystems (
        system:
        let
          context = mkContext system;
        in
        {
          default = context.craneLib.cargoTest (
            context.commonArgs
            // {
              inherit (context) cargoArtifacts;
            }
          );
          message-cli-accepts-one-nota-record-and-prints-one-nota-reply =
            context.messageConstraintCheck "message-cli-accepts-one-nota-record-and-prints-one-nota-reply" ./scripts/message-cli-accepts-one-nota-record-and-prints-one-nota-reply;
          message-runtime-cannot-reference-retired-terminal-brand =
            context.sourceConstraintCheck "message-runtime-cannot-reference-retired-terminal-brand" ./scripts/message-runtime-cannot-reference-retired-terminal-brand;
          message-cli-sends-router-signal-without-local-ledger = context.craneLib.cargoTest (
            context.commonArgs
            // {
              inherit (context) cargoArtifacts;
              cargoTestExtraArgs = "--test message command_line_send_routes_signal_frame_without_writing_local_ledger -- --exact";
            }
          );
          message-cli-inbox-uses-router-signal-not-local-ledger = context.craneLib.cargoTest (
            context.commonArgs
            // {
              inherit (context) cargoArtifacts;
              cargoTestExtraArgs = "--test message command_line_inbox_routes_signal_frame_without_reading_local_ledger -- --exact";
            }
          );
          message-daemon-routes-cli-clients-through-kameo-ledger = context.craneLib.cargoTest (
            context.commonArgs
            // {
              inherit (context) cargoArtifacts;
              cargoTestExtraArgs = "--test daemon cli_clients_route_messages_through_daemon_actor_state -- --exact";
            }
          );
        }
      );

      devShells = forSystems (
        system:
        let
          context = mkContext system;
        in
        {
          default = context.pkgs.mkShell {
            packages = [
              context.toolchain
              context.pkgs.jujutsu
              context.pkgs.nix
             
            ];
          };
        }
      );
    };
}
