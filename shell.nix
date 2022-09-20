with import ./common.nix;

pkgs.stdenv.mkDerivation {
  name = "risuto";
  buildInputs = (
    (with pkgs; [
      cacert
      diesel-cli
      mdbook
      rust-analyzer
    ]) ++
    (with rustNightlyChannel; [
      cargo
      rust
    ])
  );
}
