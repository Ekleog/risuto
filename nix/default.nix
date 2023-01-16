
let
  sources = import ./sources.nix;
in
import sources.nixpkgs {
  config = {
    allowUnfreePredicate = pkg: pkg.name == "androidsdk";
    android_sdk.accept_license = true;
  };
  overlays = [
    (import (sources.fenix + "/overlay.nix"))
    (self: super: {
      npmlock2nix = self.callPackage sources.npmlock2nix {};
    })
  ];
}
