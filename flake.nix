{
  description = "Symbolon: compare file locations and report how exactly they differ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
      git-hooks,
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
      # Host-only toolchain (no cross targets) for formatting and CI shells.
      hostRust = pkgs: (rustFor pkgs [ ]).override { targets = [ ]; };
      devRust =
        pkgs:
        rustFor pkgs [
          "rust-src"
          "rust-analyzer"
          "llvm-tools-preview"
        ];

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

      # Language servers for any LSP editor; versions match the toolchain and CI. rust-analyzer
      # comes with the Rust toolchain (devRust). See docs/development/editor.md.
      languageServers =
        pkgs: with pkgs; [
          nixd
          taplo
          svelte-language-server
          vtsls
          vscode-langservers-extracted
          yaml-language-server
          marksman
          rumdl
          typos-lsp
          just-lsp
        ];

      docsTools =
        pkgs: with pkgs; [
          mdbook
          mdbook-mermaid
          lychee
        ];

      # treefmt reads treefmt.toml; Prettier is taken from the frontend's node_modules.
      formatters =
        pkgs: with pkgs; [
          (hostRust pkgs)
          nixfmt
          taplo
        ];
      treefmtFor =
        pkgs:
        pkgs.writeShellApplication {
          name = "treefmt";
          runtimeInputs = [ pkgs.treefmt ] ++ formatters pkgs;
          text = ''exec treefmt "$@"'';
        };

      # Hooks run outside the Nix sandbox (they need node_modules and the cargo registry),
      # so they are installed by the devShell and run in CI via `pre-commit run`, not `nix flake check`.
      hooksFor =
        pkgs:
        git-hooks.lib.${pkgs.stdenv.hostPlatform.system}.run {
          src = ./.;
          default_stages = [ "pre-commit" ];
          hooks = {
            treefmt = {
              enable = true;
              packageOverrides.treefmt = pkgs.treefmt;
              settings.formatters = formatters pkgs;
            };
            typos.enable = true;
            rumdl.enable = true;
            statix.enable = true;
            deadnix.enable = true;
            actionlint.enable = true;
            zizmor.enable = true;
            check-merge-conflicts.enable = true;
            detect-private-keys.enable = true;
            convco.enable = true;
            pre-push-rust = {
              enable = true;
              name = "clippy, tests, unused dependencies";
              entry = "just lint-rust test";
              pass_filenames = false;
              stages = [ "pre-push" ];
            };
            pre-push-ui = {
              enable = true;
              name = "frontend lint, typecheck, tests";
              entry = "just lint-ui test-ui";
              pass_filenames = false;
              stages = [ "pre-push" ];
            };
          };
        };
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
            inherit ((builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package) version;
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

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) symbolon;
      });

      devShells = forAllSystems (
        pkgs:
        let
          hooks = hooksFor pkgs;
        in
        {
          default = pkgs.mkShell {
            packages =
              with pkgs;
              [
                (devRust pkgs)
                (treefmtFor pkgs)
                just
                cargo-deny
                cargo-llvm-cov
                cargo-nextest
                cargo-shear
                cargo-tauri
                cargo-zigbuild
                zig
                git-cliff
                gh
                jq
                android-tools
                nodejs_24
                pnpm
                pkg-config
                gobject-introspection
              ]
              ++ hooks.enabledPackages
              ++ languageServers pkgs
              ++ docsTools pkgs
              ++ tauriLibs pkgs;

            shellHook = ''
              export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:''${XDG_DATA_DIRS:-}"
              ${hooks.shellHook}
            '';
          };

          # Lean shells for CI, so jobs download and cache only what they use.
          # `ci`: git hooks and docs (no language servers, Tauri libraries, adb or cross toolchain).
          ci = pkgs.mkShellNoCC {
            packages =
              with pkgs;
              [
                (hostRust pkgs)
                (treefmtFor pkgs)
                just
                nodejs_24
                pnpm
              ]
              ++ hooks.enabledPackages
              ++ docsTools pkgs;

            inherit (hooks) shellHook;
          };

          # `android`: cross-compiling the Android agent.
          android = pkgs.mkShell {
            packages = with pkgs; [
              (rustFor pkgs [ ])
              just
              cargo-zigbuild
              zig
            ];
          };
        }
      );

      formatter = forAllSystems treefmtFor;
    };
}
