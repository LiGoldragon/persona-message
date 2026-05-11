{
  description = "Persona message NOTA proxy.";

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
          sourceConstraintCheck =
            name: script:
            pkgs.runCommand name { } ''
              set -euo pipefail

              export PATH=${pkgs.lib.makeBinPath [ pkgs.ripgrep ]}:$PATH
              ${pkgs.bash}/bin/bash ${script} ${./.}

              touch "$out"
            '';
          cargoTest =
            testName: craneLib.cargoTest (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoTestExtraArgs = "--test message ${testName} -- --exact";
              }
            );
          context = {
            inherit
              pkgs
              toolchain
              craneLib
              commonArgs
              cargoArtifacts
              sourceConstraintCheck
              cargoTest
              ;
          };
        in
        context;
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
          message-runtime-cannot-reference-retired-terminal-brand =
            context.sourceConstraintCheck "message-runtime-cannot-reference-retired-terminal-brand" ./scripts/message-runtime-cannot-reference-retired-terminal-brand;
          message-proxy-cannot-own-local-ledger =
            context.sourceConstraintCheck "message-proxy-cannot-own-local-ledger" ./scripts/message-proxy-cannot-own-local-ledger;
          message-cli-sends-router-signal-without-local-ledger =
            context.cargoTest "command_line_send_routes_signal_frame_without_writing_local_ledger";
          message-cli-inbox-uses-router-signal-not-local-ledger =
            context.cargoTest "command_line_inbox_routes_signal_frame_without_reading_local_ledger";
          message-cli-requires-router-socket =
            context.cargoTest "command_line_send_requires_router_socket";
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
