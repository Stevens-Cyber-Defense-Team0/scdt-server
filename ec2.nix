{ pkgs, modulesPath, challd, ctf_archive, frontend, ... }:
{
	imports = [ (modulesPath + "/virtualisation/amazon-image.nix") ];

	# lock down nix daemon
	nix.settings.allowed-users = [ "root" "james" ];

	networking.hostName = "scdt";
	networking.firewall = {
		enable = true;
		allowedTCPPorts = [ 80 443 ];
		allowedUDPPorts = [ 51820 ];
	};

	# enable ip forwarding
	boot.kernel.sysctl."net.ipv4.ip_forward" = 1;

	# don't automatically assign eth42 an address
	networking.interfaces.eth42.useDHCP = false;

	users.groups.challd = {};

	users.mutableUsers = false;
	users.users = {
		root = {
			isSystemUser = true;
			openssh.authorizedKeys.keys = [
				"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOaXXX1gINYT/j5nRZL0XePCd+fyZdp2vLOn3eKXDp15"
				"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9UrLap0mk8rqCBVBvWFOzgrCSU1xPS3sqfCMtq0f/9"
			];
		};

		james = {
			isNormalUser = true;
			extraGroups = [ "docker" ];
			openssh.authorizedKeys.keys = [
				"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOaXXX1gINYT/j5nRZL0XePCd+fyZdp2vLOn3eKXDp15"
				"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9UrLap0mk8rqCBVBvWFOzgrCSU1xPS3sqfCMtq0f/9"
			];
			packages = with pkgs; [
				vim
				zip
				unzip
				p7zip
				htop
				wget
				file
			];
		};

		ctf_archive = {
			isNormalUser = true;
			extraGroups = [ "challd" ];
		};
	};

	services.nginx = {
		enable = true;
		recommendedProxySettings = true;
		recommendedTlsSettings = true;
		recommendedGzipSettings = true;

		appendHttpConfig = ''
			underscores_in_headers on;
			limit_req_zone $binary_remote_addr zone=sensitive:10m rate=1r/s;
		'';

		virtualHosts."scdt.club" = {
			enableACME = true;
			forceSSL = true;

			locations."/" = {
				root = frontend;
			};

			locations."~ ^/api/(?:auth|guest)" = {
				proxyPass = "http://127.0.0.1:8080";
				extraConfig = ''
					limit_req zone=sensitive burst=5;
				'';
			};
			locations."~ ^/(?:api|swagger)" = {
				proxyPass = "http://127.0.0.1:8080";
			};
		};
	};

	services.postgresql = {
		enable = true;
		ensureDatabases = [ "ctf_archive" ];
		ensureUsers = [{
			name = "ctf_archive";
			ensureDBOwnership = true;
		}];
		authentication = pkgs.lib.mkOverride 10 ''
			local postgres postgres trust
			local ctf_archive ctf_archive trust
		'';
	};

	security.acme = {
		acceptTerms = true;
		defaults.email = "jconnoll1@stevens.edu";
	};

	systemd.services.challd = {
		wantedBy = [ "multi-user.target" ];

		environment.SOCKET_PATH = "/etc/challd/challd.sock";
		environment.RUST_LOG = "info";
		path = with pkgs; [
			iptables
			iproute2
		];
		serviceConfig = {
			User = "root";
			ExecStart = ''${pkgs.writeScript "launch"
				"#!/bin/sh\nmkdir -p /etc/challd && chown root:challd /etc/challd && chmod 730 /etc/challd && rm -f /etc/challd/challd.sock && ${challd}/bin/challd"
			}'';
			Restart = "on-failure";
		};
	};

	systemd.services.ctf_archive = {
		before = [ "nginx.service" ];
		after = [ "postgresql.service" ];
		requisite = [ "postgresql.service" "challd.service" ];
		wantedBy = [ "multi-user.target" ];

		environment.RUST_LOG = "info";
		serviceConfig = {
			User = "ctf_archive";
			ExecStart = "${ctf_archive}/bin/ctf_archive";
			Restart = "on-failure";
		};
	};

	virtualisation.docker.enable = true;
	virtualisation.docker.daemon.settings = {
		runtimes.runsc.path = "${pkgs.gvisor}/bin/runsc";
	};

	nix.gc = {
		automatic = true;
		dates = "daily";
		options = "--delete-older-than 3d";
	};
	nix.optimise = {
		automatic = true;
		dates = [ "daily" ];
	};
	nix.settings.experimental-features = [ "nix-command" "flakes" ];

	services.journald.extraConfig = "SystemMaxUse=64M";

	system.stateVersion = "24.05";
}
