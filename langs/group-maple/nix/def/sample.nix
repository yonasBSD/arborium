{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    pkg-config
    openssl
  ];

  shellHook = ''
    echo "Development environment ready!"
    export RUST_BACKTRACE=1
  '';
}
