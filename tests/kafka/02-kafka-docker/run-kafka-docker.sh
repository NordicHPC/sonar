#!/bin/bash
#
# This will run Kafka in this directory on a custom config file and create some topics if necessary.
# See README.md for more information.
if [ -z $HOSTNAME ]; then
    HOSTNAME=$(hostname)
    export HOSTNAME
fi

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

HOST_IP=$(ping $(hostname) -c 1 | head -1 | sed "s#.*(\([0-9\.]\+[0-9\.]\+[0-9.]\+[0-9]\+\)).*#\1#g")
if [ -z $HOST_IP ]; then
    echo "Could not identify (default) host ip via:"
    echo "    ip r | grep "default via" | cut -d' ' -f3"
    pl
fi
export HOST_IP

SCRIPT_DIR=$(realpath -L $(dirname $0))
cd $SCRIPT_DIR

( cd ./ssl ; make all )

CONTAINER_IP_ADDRESS=$(docker inspect --format "{{ .NetworkSettings.Networks.kafka_default.IPAddress }}" $CONTAINER_ID)
if [ -n $CONTAINER_IP_ADDRESS ]; then
    $DOCKER_COMPOSE_CMD down
fi

echo "HOSTNAME: ${HOSTNAME}"
echo "Using HOST_IP=${HOST_IP}"

HOSTNAME=$HOSTNAME HOST_IP=$HOST_IP $DOCKER_COMPOSE_CMD up -d
CONTAINER_ID=$(docker inspect --format "{{ .Id }}" kafka-broker)
CONTAINER_IP_ADDRESS=$(docker inspect --format "{{ .NetworkSettings.Networks.kafka_default.IPAddress }}" $CONTAINER_ID)

# NO NEED TO CREATE TOPICS -- they will be automatically created, when clients send to the broker

echo "READY"
