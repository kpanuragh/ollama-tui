{
  description = "A feature-rich, terminal-based user interface for interacting with Ollama, written in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        
        # Use the Rust version specified in your project (edition 2024 suggests recent Rust)
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" ];
        };

        # Define the native build inputs needed for your dependencies
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        buildInputs = with pkgs; [
          openssl
          sqlite
        ] ++ lib.optionals stdenv.isDarwin [
          # macOS specific dependencies
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        # Build the Rust package
        ollama-tui = pkgs.rustPlatform.buildRustPackage {
          pname = "ollama-tui";
          version = "1.2.7";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          # Set environment variables for native dependencies
          OPENSSL_NO_VENDOR = 1;
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

          meta = with pkgs.lib; {
            description = "A feature-rich, terminal-based user interface for interacting with Ollama";
            homepage = "https://github.com/kpanuragh/ollama-tui";
            license = licenses.mit; # Adjust based on your LICENSE file
            maintainers = [ ];
            platforms = platforms.all;
            mainProgram = "ollama-tui";
          };
        };

      in
      {
        # The main package output
        packages = {
          default = ollama-tui;
          ollama-tui = ollama-tui;
        };

        # Development shell with all necessary tools
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            # Additional development tools
            rust-analyzer
            rustfmt
            cargo-watch
            cargo-edit
          ]);

          shellHook = ''
            echo "Development environment for ollama-tui"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build --release  # Build the project"
            echo "  cargo run             # Run the project"
            echo "  cargo watch -x run     # Run with file watching"
            echo "  cargo test            # Run tests"
          '';

          # Environment variables for development
          OPENSSL_NO_VENDOR = 1;
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        # Make the app easily runnable
        apps.default = flake-utils.lib.mkApp {
          drv = ollama-tui;
          name = "ollama-tui";
        };

        # Formatter for nix files
        formatter = pkgs.nixpkgs-fmt;
      });
}
