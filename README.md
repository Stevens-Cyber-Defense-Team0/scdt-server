# SCDT CTF Archive

## Architecture Overview
The SCDT CTF Archive has the following components:
- `ctf_archive`: axum application that handles API requests
- Nginx: hosts static content and acts as a reverse proxy for `ctf_archive` routes
- PostgreSQL: database for `ctf_archive`
- `challd`: privileged daemon that runs a UNIX socket at `/etc/challd/challd.sock`
	- responsible for starting/stopping gVisor containers for challenges and WireGuard/iptables configuration

## `ctf_archive`
This is the server that handles authentication and API requests.
Documentation can be found [here](https://scdt.club/swagger).

### Admin Users
Admin users have an `id` and a P-256 signing key.
The server generates a random short-lived challenge that the admin user must produce a valid ECDSA signature of with their signing key in order to authenticate.

Admin users have various permissions that allow them to call certain API routes.
Notably, admin users can generate guest codes.
Guest codes are a temporary numeric code used to get an API key for a guest user.

### Guest Users
Guest users can browse the archive and start challenges.
When a guest starts a challenge, the associated gVisor container is started and a WireGuard configuration is generated that is capable of connecting to the container.

### Challenge Containers
Challenge containers often run vulnerable code and have intentional RCE vulnerabilities, as such we've architected this system in a very paranoid way.

The following security measures are in place:
- The logic for starting and managing containers is done inside the `challd` daemon, ensuring `ctf_archive` runs with the absolute minimum privilege possible.
- Containers are run using gVisor, a tool specifically designed for running untrusted code safely.
- A WireGuard configuration must be used in order to connect to a challenge container, this ensures that only reasonably trustworthy users can interact with vulnerable code.
- Challenge containers have various resource limits and are automatically destroyed after a certain amount of time to prevent denial of service.

## `challd`
This service runs on a privileged user and can only be interfaced with by users that are part of the `challd` group.
The `challd` service listens on a UNIX socket and uses a custom binary protocol to pass messages.
The purpose of `challd` is to ensure that the API server doesn't run with higher privilege than should be necessary.

Internally, `challd` does the following:
- Keeps track of running containers, WireGuard peers, and their associated IP addresses using a lock-free data structure.
- Automatically destroys containers and WireGuard peers when they've been running too long.
- Handles logic for graceful shutdown of containers and WireGuard peers when it receives a `SIGINT` or `SIGTERM`.
