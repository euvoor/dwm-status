#!/bin/bash

cargo build --release

rm -f /data/usr/local/bin/dwm_status
ln -s `pwd`/target/release/dwm_status /data/usr/local/bin/dwm_status

nohup dwm_status > /dev/null &
