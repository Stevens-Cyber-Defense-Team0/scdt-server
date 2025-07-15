{
	inputs = {
		nixpkgs.url = "github:nixos/nixpkgs/release-25.05";
		flake-utils.url = "github:numtide/flake-utils";
		crane.url = "github:ipetkov/crane";
		deploy-rs = {
			url = "github:serokell/deploy-rs";
			inputs.nixpkgs.follows = "nixpkgs";
		};
	};

	outputs = { self, nixpkgs, flake-utils, crane, deploy-rs }:
		flake-utils.lib.eachDefaultSystem (system:
			let pkgs = import nixpkgs {
				inherit system;
			};
			craneLib = crane.mkLib pkgs;
			challd = craneLib.buildPackage {
				pname = "challd";
				src = craneLib.cleanCargoSource ./challd;
				cargoExtraArgs = "--features gvisor --features challd_group --features domain_name";
			};
			frontend = pkgs.stdenvNoCC.mkDerivation {
				pname = "frontend";
				version = "0.0.1";
				src = pkgs.fetchFromGitHub {
					owner = "Stevens-Cyber-Defense-Team0";
					repo = "Ducklink";
					rev = "3bc2a9c5c6c0a9cc3f4d7f5f2211783b072d1b9e";
					sha256 = "sha256-LAC19zT0fEolAHe+poG8YsQUzBwhauhj8s6Dk5agApk=";
				};
				installPhase = "rm README.md && mkdir -p $out && mv * $out";
			};
			ctf_archive = craneLib.buildPackage {
				pname = "ctf_archive";
				src = pkgs.lib.cleanSourceWith {
					src = craneLib.path ./ctf_archive;
					filter = path: type: (craneLib.filterCargoSources path type)
						|| (builtins.match ".*\\.(html|sql)$" path != null);
				};
				buildInputs = with pkgs; [
					postgresql
				];
				cargoExtraArgs = "--features production";
			}; in {
				devShell = pkgs.mkShell {
					packages = with pkgs; [
						cargo
						clippy
						cargo-outdated
						deploy-rs.packages.${system}.default
						diesel-cli
						postgresql
						(python3.withPackages (p: with p; [
							requests
							pycryptodome
						]))
					];
				};

				packages.challd = challd;
				packages.ctf_archive = ctf_archive;
				packages.frontend = frontend;
		}) // {
			deploy.nodes.aws = {
				hostname = "scdt.club";

				profiles.system = {
					sshUser = "root";
					user = "root";

					path = deploy-rs.lib.x86_64-linux.activate.nixos (nixpkgs.lib.nixosSystem {
						system = "x86_64-linux";

						modules = [ ./ec2.nix ];
						specialArgs = {
							challd = self.packages.x86_64-linux.challd;
							ctf_archive = self.packages.x86_64-linux.ctf_archive;
							frontend = self.packages.x86_64-linux.frontend;
						};
					});
				};
			};

			checks = builtins.mapAttrs (system: deployLib: deployLib.deployChecks self.deploy) deploy-rs.lib;
		};
}
