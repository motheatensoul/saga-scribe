{
  description = "Saga-Scribe development environment";

  # `nix develop` gives bun, the Rust toolchain pinned in rust-toolchain.toml and the
  # system libraries Tauri and libxml2 need, so that `bun install`, `bun run tauri dev`
  # and `bun run check:all` work. Packaging is done elsewhere (Flatpak, CI).

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      # nixpkgs-unstable no longer supports Intel macOS (x86_64-darwin).
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        f (import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        }));
    in
    {
      devShells = forAllSystems (pkgs:
        let
          inherit (pkgs) lib stdenv;
          # Read from the same file rustup uses, so the two can't drift apart.
          rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
            extensions = [ "rust-src" ];
          };
          # Tauri on Linux (https://v2.tauri.app/start/prerequisites/#linux)
          linuxLibs = with pkgs; [
            webkitgtk_4_1
            gtk3
            glib
            cairo
            pango
            gdk-pixbuf
            harfbuzz
            at-spi2-atk
            libsoup_3
            librsvg
            openssl
          ];
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = [
              rust
              pkgs.bun
              pkgs.pkg-config
              # The libxml crate generates its bindings with bindgen, which needs libclang.
              pkgs.rustPlatform.bindgenHook
            ] ++ lib.optionals stdenv.hostPlatform.isLinux [ pkgs.patchelf ];

            buildInputs = [ pkgs.libxml2 ]
              ++ lib.optionals stdenv.hostPlatform.isLinux linuxLibs
              ++ lib.optionals stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];

            shellHook = lib.optionalString stdenv.hostPlatform.isLinux ''
              # GTK's settings schemas, which the file dialogs need at run time
              export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
            '';
          };
        });
    };
}
