{
  description = "Symbolon: compare file locations and report how exactly they differ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );

      toolchainFile = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain;
      rustFor =
        pkgs: extensions:
        pkgs.rust-bin.stable.${toolchainFile.channel}.minimal.override {
          extensions = toolchainFile.components ++ extensions;
          inherit (toolchainFile) targets;
        };

      # Libraries needed to build and run the Tauri app on Linux.
      tauriLibs =
        pkgs: with pkgs; [
          at-spi2-atk
          atkmm
          cairo
          gdk-pixbuf
          glib
          gtk3
          harfbuzz
          librsvg
          libsoup_3
          pango
          webkitgtk_4_1
          openssl
        ];
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          craneLib = (crane.mkLib pkgs).overrideToolchain (p: rustFor p [ ]);
          src = craneLib.cleanCargoSource ./.;
          commonArgs = {
            inherit src;
            strictDeps = true;
            pname = "symbolon";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
            cargoExtraArgs = "--locked -p symbolon-cli";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        in
        {
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.symbolon;
          symbolon = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              # Tests run under nextest in CI and `just test`; skip the duplicate run here.
              doCheck = false;
              meta = {
                description = "Compare file locations and report how exactly they differ";
                license = pkgs.lib.licenses.gpl3Plus;
                mainProgram = "symbolon";
              };
            }
          );
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages =
            with pkgs;
            [
              (rustFor pkgs [
                "rust-src"
                "rust-analyzer"
              ])
              just
              cargo-deny
              cargo-nextest
              cargo-tauri
              cargo-zigbuild
              zig
              git-cliff
              android-tools
              nodejs_24
              pnpm
              pkg-config
              gobject-introspection
            ]
            ++ tauriLibs pkgs;

          shellHook = ''
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:''${XDG_DATA_DIRS:-}"
          '';
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
