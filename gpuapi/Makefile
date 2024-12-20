CFLAGS=-g -O2 -Wall -fPIC
LDFLAGS=-ldl

.PHONY: default

default:
	@echo "Choose a specific target: sonar-nvidia.a"

sonar-nvidia.a: sonar-nvidia.o
	ar r sonar-nvidia.a sonar-nvidia.o
	ranlib sonar-nvidia.a

sonar-nvidia.o: sonar-nvidia.c Makefile
	$(CC) -c $(CFLAGS) -o sonar-nvidia.o sonar-nvidia.c
