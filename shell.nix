{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust development
    rustc
    cargo
    rustfmt
    clippy

    # Required for Tantivy
    pkg-config
    openssl
    openssl.dev
    zstd
    zlib
  ];

  # Environment variables
  shellHook = ''
    export RUST_BACKTRACE=1
    export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
  '';
}
