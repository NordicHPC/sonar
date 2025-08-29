#!/bin/bash
#
# This will cleanly stop a Kafka that was started by run-kafka-docker.sh.  See README.md for more
# information.

# Find latest docker version
docker compose version
DOCKER_COMPOSE_CMD=
if [ $? -eq 0 ]; then
    DOCKER_COMPOSE_CMD="docker compose"
else
    docker-compose --version
    if [ $? -eq 0 ]; then
        DOCKER_COMPOSE_CMD="docker-compose"
    else
        echo "Docker compose is not available.  Please install first."
        exit 1
    fi
fi

$DOCKER_COMPOSE_CMD down

