#!/bin/sh
# Substitute POTATO_PORT in supervisord template and start supervisord.
# Default port 8088; override at runtime with -e POTATO_PORT=3000.
set -e

is_auth_enabled() {
  if [ "${POTATO_AUTH_ENABLED+x}" ]; then
    flag="${POTATO_AUTH_ENABLED}"
  else
    flag="${COPAW_AUTH_ENABLED:-}"
  fi
  flag="$(printf '%s' "$flag" | tr '[:upper:]' '[:lower:]')"
  [ "$flag" = "true" ] || [ "$flag" = "1" ] || [ "$flag" = "yes" ]
}

warn_if_auth_off_container_bind() {
  if is_auth_enabled; then
    return
  fi

  cat >&2 <<EOF
============================================================
SECURITY NOTICE: Potato is running in Docker without authentication.

Potato cannot verify whether access to the service is limited to a trusted
network. Anyone who can reach the service may access Potato APIs without login.

Recommended:
  - Restrict access to a trusted network or protected environment.
  - Enable authentication with POTATO_AUTH_ENABLED=true if untrusted users or
    processes may reach the service.
============================================================
EOF
}

# Auto-initialize if config.json is missing (bind mount with empty directory).
if [ ! -f "${POTATO_WORKING_DIR}/config.json" ]; then
  echo "⚠️  No config.json found in ${POTATO_WORKING_DIR}"
  echo "📦 Running initialization..."
  potato init --defaults --accept-security
  echo "✅ Initialization complete!"
else
  echo "✓ Config found in ${POTATO_WORKING_DIR}, skipping initialization."
fi

export POTATO_PORT="${POTATO_PORT:-8088}"
warn_if_auth_off_container_bind
envsubst '${POTATO_PORT}' \
  < /etc/supervisor/conf.d/supervisord.conf.template \
  > /etc/supervisor/conf.d/supervisord.conf
exec /usr/bin/supervisord -c /etc/supervisor/conf.d/supervisord.conf
