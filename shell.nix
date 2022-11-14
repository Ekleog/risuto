with import ./common.nix;

pkgs.stdenv.mkDerivation {
  name = "risuto";
  buildInputs = (
    (with pkgs; [
      cacert
      mdbook
      openssl
      pkgconfig
      rust-analyzer
      nodePackages.sass
      sqlx-cli
      trunk
    ]) ++
    (with rustNightlyChannel; [
      cargo
      (rust.override {
        targets = ["wasm32-unknown-unknown"];
      })
    ])
  );
}
