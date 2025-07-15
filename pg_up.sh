#!/bin/sh

docker run -d --rm --name scdt-postgres -e POSTGRES_USER=ctf_archive -e POSTGRES_DB=ctf_archive -e POSTGRES_PASSWORD=hunter2 -p 127.0.0.1:5432:5432/tcp postgres
