{ pkgs, ... }: {
  languages.rust.enable = true;

  packages = [ pkgs.cargo-insta ];
}
