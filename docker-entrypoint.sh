#!/bin/bash
set -e
export RUST_BACKTRACE=1

ARGS=()

# Location / position
LAT="${READSB_LAT:-$LAT}"
LON="${READSB_LON:-$LON}"
[ -n "$LAT" ] && ARGS+=("--lat" "$LAT")
[ -n "$LON" ] && ARGS+=("--lon" "$LON")

# SDR device configuration
[ -n "$READSB_DEVICE_TYPE" ] && ARGS+=("--device-type" "$READSB_DEVICE_TYPE")
[ -n "$READSB_DEVICE" ] && ARGS+=("--device" "$READSB_DEVICE")

GAIN="${READSB_GAIN:-$GAIN}"
PPM="${READSB_PPM:-$PPM}"
if [ -n "$GAIN" ] && [ "$GAIN" != "auto" ]; then
    ARGS+=("--gain" "$GAIN")
fi
[ -n "$PPM" ] && ARGS+=("--ppm" "$PPM")
[ -n "$READSB_FREQ" ] && ARGS+=("--freq" "$READSB_FREQ")

# RTL-TCP specific
[ -n "$READSB_RTLTCP_DIRECT_SAMP" ] && ARGS+=("--rtltcp-direct-samp" "$READSB_RTLTCP_DIRECT_SAMP")
[ -n "$READSB_RTLTCP_OFFSET_TUNE" ] && ARGS+=("--rtltcp-offset-tune")
[ -n "$READSB_RTLTCP_BIAS_TEE" ] && ARGS+=("--rtltcp-bias-tee")

# Networking — support both READSB_NET and bare NET
NET_ENABLED="${READSB_NET:-$NET}"
if [ "$NET_ENABLED" = "yes" ] || [ "$NET_ENABLED" = "1" ] || [ "$NET_ENABLED" = "true" ]; then
    ARGS+=("--net")
fi
[ -n "$READSB_NET_BO_PORT" ] && ARGS+=("--net-bo-port" "$READSB_NET_BO_PORT")
[ -n "$READSB_NET_RI_PORT" ] && ARGS+=("--net-ri-port" "$READSB_NET_RI_PORT")
[ -n "$READSB_NET_SBS_PORT" ] && ARGS+=("--net-sbs-port" "$READSB_NET_SBS_PORT")
[ -n "$READSB_NET_BIND_ADDRESS" ] && ARGS+=("--net-bind-address" "$READSB_NET_BIND_ADDRESS")

# JSON output
[ -n "$READSB_JSON_DIR" ] && ARGS+=("--json-dir" "$READSB_JSON_DIR")
[ -n "$READSB_JSON_GLOBE_INDEX" ] && ARGS+=("--json-globe-index")
[ -n "$READSB_JSON_RELIABLE" ] && ARGS+=("--json-reliable" "$READSB_JSON_RELIABLE")
[ -n "$READSB_JSON_TRACE_INTERVAL" ] && ARGS+=("--json-trace-interval" "$READSB_JSON_TRACE_INTERVAL")

# File input
[ -n "$READSB_IFILE" ] && ARGS+=("--ifile" "$READSB_IFILE")
[ -n "$READSB_IFORMAT" ] && ARGS+=("--iformat" "$READSB_IFORMAT")

# Performance
[ -n "$READSB_DECODE_THREADS" ] && ARGS+=("--decode-threads" "$READSB_DECODE_THREADS")
[ -n "$READSB_AGGRESSIVE" ] && ARGS+=("--aggressive")

# Debug / verbosity
[ -n "$READSB_DEBUG_NET" ] && ARGS+=("--debug-net")
[ -n "$READSB_DEBUG_CPR" ] && ARGS+=("--debug-cpr")
[ -n "$READSB_DEBUG_GARBAGE" ] && ARGS+=("--debug-garbage")
[ -n "$READSB_DEBUG_API" ] && ARGS+=("--debug-api")
[ -n "$READSB_QUIET" ] && ARGS+=("--quiet")

# Extra user-specified args
[ -n "$READSB_EXTRA_ARGS" ] && ARGS+=($READSB_EXTRA_ARGS)

# Append any CLI args passed to docker run
ARGS+=("$@")

echo "Starting readsb-rs: ${ARGS[*]}"
exec /usr/local/bin/readsb "${ARGS[@]}"
