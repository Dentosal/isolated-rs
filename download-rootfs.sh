#!/bin/sh

wget https://dl-cdn.alpinelinux.org/alpine/v3.14/releases/x86_64/alpine-minirootfs-3.14.0-x86_64.tar.gz
mkdir rootfs
tar -xf alpine-minirootfs-3.14.0-x86_64.tar.gz -C ./rootfs
