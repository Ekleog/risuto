with import ./common.nix;

pkgs.stdenv.mkDerivation {
  name = "risuto";
  buildInputs = (
    (with pkgs; [
      cacert
      mdbook
      rust-analyzer
    ]) ++
    (with rustNightlyChannel; [
      cargo
      rust
    ])
  );
}
