#!/bin/sh
# Generate runtime config from environment variables.
# Only non-empty vars are emitted, so the Angular defaults apply otherwise.

JS=/usr/share/nginx/html/config.js

cat > "$JS" <<'HEADER'
(function(){
  var e = {};
HEADER

[ -n "$API_SERVER" ]        && echo "  e.API_SERVER = \"$API_SERVER\";" >> "$JS"
[ -n "$AUTH_PROVIDER_BASE" ] && echo "  e.AUTH_PROVIDER_BASE = \"$AUTH_PROVIDER_BASE\";" >> "$JS"
[ -n "$AUTH_ISSUER" ]       && echo "  e.AUTH_ISSUER = \"$AUTH_ISSUER\";" >> "$JS"
[ -n "$AUTH_CLIENT_ID" ]    && echo "  e.AUTH_CLIENT_ID = \"$AUTH_CLIENT_ID\";" >> "$JS"
[ -n "$AUTH_REDIRECT_PATH" ] && echo "  e.AUTH_REDIRECT_PATH = \"$AUTH_REDIRECT_PATH\";" >> "$JS"
[ -n "$AUTH_REDIRECT_URI" ] && echo "  e.AUTH_REDIRECT_URI = \"$AUTH_REDIRECT_URI\";" >> "$JS"
APP_VERSION="${APP_VERSION:-$(cat /etc/diraigent_version 2>/dev/null)}"
[ -n "$APP_VERSION" ]      && echo "  e.APP_VERSION = \"$APP_VERSION\";" >> "$JS"

cat >> "$JS" <<'FOOTER'
  globalThis.__env = e;
})();
FOOTER

exec nginx -g "daemon off;"
