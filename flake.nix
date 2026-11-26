{
  # Modelled on CraftCloud's flake.nix, itself on craft-launcher's (MIT OR Apache-2.0).
  #
  #   nix run github:jonasbove/pingcraft      # the desktop app
  #   nix develop                             # a shell for cargo
  #
  # Untried: written without Nix at hand; .github/workflows/nix.yml evaluates it in CI. flake.lock is made by the
  # first `nix flake lock` and committed then.
  description = "PingCraft: all your chats in one inbox, on your own server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        lib = pkgs.lib;

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        # Packages build with the minimal toolchain (compiler and std only), so the binary keeps no toolchain
        # alive through std's source paths (see postInstall).
        rustMinimal = pkgs.rust-bin.stable.latest.minimal;
        rustPlatform = pkgs.makeRustPlatform { cargo = rustMinimal; rustc = rustMinimal; };

        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

        # The window (winit, wgpu/GL) needs no GTK; the keychain is pure-Rust D-Bus (zbus). SQLite is the
        # system's (matrix-sdk's stores; nixpkgs' should be 3.54.0 or newer, RUSTSEC-2026-0459).
        runtimeLibs = with pkgs; [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          libx11
          libxcursor
          libxrandr
          libxi
          libxcb
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [ rust pkgs.pkg-config ];
          buildInputs = runtimeLibs ++ [ pkgs.sqlite ];
          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
        };

        packages.default = rustPlatform.buildRustPackage {
          pname = "pingcraft";
          inherit version;
          # Only what the Rust workspace builds from, plus the logo it embeds and the desktop files.
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./.cargo ./crates ./apps ./xtask ./packaging/linux ./design/assets/logo ];
          };
          cargoLock = {
            lockFile = ./Cargo.lock;
            # Git dependencies (the storytold egui/winit forks, craft-libs) at their locked revisions.
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [ "-p" "pingcraft" ];
          nativeBuildInputs = [ pkgs.pkg-config pkgs.autoPatchelfHook pkgs.removeReferencesTo ];
          # libgcc_s for the unwinder, as every Rust binary needs.
          buildInputs = runtimeLibs ++ [ pkgs.sqlite pkgs.stdenv.cc.cc.lib ];
          runtimeDependencies = runtimeLibs;
          postInstall = ''
            remove-references-to -t ${rustMinimal} $out/bin/pingcraft
            id=io.github.jonasbove.pingcraft
            install -Dm644 packaging/linux/$id.desktop $out/share/applications/$id.desktop
            mkdir -p $out/share/metainfo
            sed -e "s/@VERSION@/${version}/g" -e "s/@DATE@/1970-01-01/g" \
              packaging/linux/$id.metainfo.xml.in > $out/share/metainfo/$id.metainfo.xml
            for size in 16 32 48 64 128 256 512; do
              install -Dm644 design/assets/logo/pingcraft-logo-$size.png $out/share/icons/hicolor/''${size}x''${size}/apps/$id.png
            done
          '';
          disallowedReferences = [ rustMinimal ];
          doCheck = false;
          meta.mainProgram = "pingcraft";
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/pingcraft";
        };
      });
}
