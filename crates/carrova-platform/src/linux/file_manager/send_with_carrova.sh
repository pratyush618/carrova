#!/bin/sh
# "Send with Carrova" for Nautilus (Scripts menu). Written by Carrova.
[ -x {{sh}} ] || exit 0
exec {{sh}} --send "$@"
