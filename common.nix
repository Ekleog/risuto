rec {
  pkgsSrc = builtins.fetchTarball {
    # The following is for nixos-unstable on 2022-03-15
    url = "https://github.com/NixOS/nixpkgs/archive/73ad5f9e147c0d2a2061f1d4bd91e05078dc0b58.tar.gz";
    sha256 = "01j7nhxbb2kjw38yk4hkjkkbmz50g3br7fgvad6b1cjpdvfsllds";
  };
  rustOverlaySrc = builtins.fetchTarball {
    # The following is the latest version as of 2022-03-15
    url = "https://github.com/mozilla/nixpkgs-mozilla/archive/15b7a05f20aab51c4ffbefddb1b448e862dccb7d.tar.gz";
    sha256 = "0admybxrjan9a04wq54c3zykpw81sc1z1nqclm74a7pgjdp7iqv1";
  };
  rustOverlay = import rustOverlaySrc;
  pkgs = import pkgsSrc {
    overlays = [
      rustOverlay
    ];
  };
  rustNightlyChannel = pkgs.rustChannelOf {
    date = "2022-03-15";
    channel = "nightly";
    sha256 = "0wgn87di2bz901iv2gspg935qgyzc3c2fg5jszckxl4q47jzvd8b";
  };
}
