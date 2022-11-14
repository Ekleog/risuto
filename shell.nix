with import ./common.nix;

pkgs.stdenv.mkDerivation {
  name = "risuto";
  buildInputs = (
    (with pkgs; [
      cacert
      mdbook
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
