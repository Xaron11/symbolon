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
      inputs.flake-compat.follows = "";
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
      inherit ((builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package) version;
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

      # What the app's -sys crates link on Linux; GTK brings cairo, pango, gdk-pixbuf and glib.
      tauriLibs =
        pkgs: with pkgs; [
          gtk3
          libsoup_3
          librsvg
          webkitgtk_4_1
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
          bash-language-server
          shellcheck
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
          shfmt
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
            shellcheck = {
              enable = true;
              # direnv runs .envrc; editing it to add a shell directive would make everyone re-allow it.
              excludes = [ "^\\.envrc$" ];
            };
            rumdl.enable = true;
            statix.enable = true;
            deadnix.enable = true;
            actionlint.enable = true;
            zizmor.enable = true;
            check-merge-conflicts.enable = true;
            detect-private-keys.enable = true;
            convco.enable = true;
            # Each pre-push check runs only when the pushed commits touch what it covers; CI runs
            # everything regardless.
            pre-push-rust = {
              enable = true;
              name = "clippy, tests, unused dependencies";
              entry = "just lint-rust test";
              files = ''^(crates/(?!symbolon-app/ui/)|crates/symbolon-app/ui/src/lib/api/bindings\.ts$|Cargo\.(toml|lock)$|rust-toolchain\.toml$|clippy\.toml$|justfile$|flake\.(nix|lock)$)'';
              pass_filenames = false;
              stages = [ "pre-push" ];
            };
            pre-push-ui = {
              enable = true;
              name = "frontend lint, typecheck, tests";
              entry = "just lint-ui test-ui";
              files = ''^(crates/symbolon-app/ui/|design/|docs/design/|justfile$|flake\.(nix|lock)$)'';
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
          inherit (pkgs) lib;
          craneLib = (crane.mkLib pkgs).overrideToolchain (p: rustFor p [ ]);
          license = with lib.licenses; [
            mit
            asl20
          ];

          cliArgs = {
            pname = "symbolon";
            inherit version;
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
            cargoExtraArgs = "--locked -p symbolon-cli";
            # Tests run under nextest in CI and `just test`.
            doCheck = false;
          };

          # The desktop app: Tauri's own build (frontend, then Rust) through nixpkgs' hooks, with
          # the toolchain from rust-toolchain.toml.
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustFor pkgs [ ];
            rustc = rustFor pkgs [ ];
          };
          uiRoot = "crates/symbolon-app/ui";
          symbolon = craneLib.buildPackage (
            cliArgs
            // {
              # Dependencies are cached per lockfile. Only `cargo build`: crane's default also checks
              # and builds every test and bench target, which the package does not use.
              cargoArtifacts = craneLib.buildDepsOnly (
                cliArgs // { buildPhaseCargoCommand = "cargoWithProfile build ${cliArgs.cargoExtraArgs}"; }
              );
              nativeBuildInputs = [ pkgs.installShellFiles ];
              postInstall = lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
                $out/bin/symbolon man --out man
                installManPage man/*.1
                installShellCompletion --cmd symbolon \
                  --bash <($out/bin/symbolon completions bash) \
                  --fish <($out/bin/symbolon completions fish) \
                  --zsh <($out/bin/symbolon completions zsh)
              '';
              meta = {
                description = "Compare file locations and report how exactly they differ";
                inherit license;
                mainProgram = "symbolon";
              };
            }
          );
          symbolon-app = rustPlatform.buildRustPackage (finalAttrs: {
            pname = "symbolon-app";
            inherit version;
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
                ./design
                # The token build writes this page; its directory must exist.
                ./docs/design/tokens.md
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            buildAndTestSubdir = "crates/symbolon-app";
            pnpmRoot = uiRoot;
            pnpmDeps = pkgs.fetchPnpmDeps {
              inherit (finalAttrs) pname version src;
              sourceRoot = "${finalAttrs.src.name}/${uiRoot}";
              fetcherVersion = 5;
              hash = "sha256-/5GEZR7+cST/YuutYQ8cWkU/Y/XF1vrztQs8vJj1CCc=";
            };
            nativeBuildInputs = with pkgs; [
              cargo-tauri.hook
              nodejs_24
              pnpm
              pnpmConfigHook
              pkg-config
              wrapGAppsHook3
            ];
            buildInputs = tauriLibs pkgs;
            # Tests run under nextest in CI and `just test`.
            doCheck = false;
            meta = {
              description = "Desktop app to compare file locations and see how exactly they differ";
              inherit license;
              mainProgram = "symbolon-app";
              platforms = lib.platforms.linux;
            };
          });
        in
        {
          inherit symbolon symbolon-app;
          default = symbolon;
        }
      );

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) symbolon symbolon-app;
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
                cargo-mutants
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
