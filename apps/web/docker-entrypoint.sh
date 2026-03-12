#!/bin/sh
# Generate runtime config from environment variables.
# Any env var left unset produces no key, so the Angular defaults apply.

cat > /usr/share/nginx/html/config.js <<EOF
(function(){
  var e = {};
  ${API_SERVER:+e.API_SERVER = "$API_SERVER";}
  ${AUTH_PROVIDER_BASE:+e.AUTH_PROVIDER_BASE = "$AUTH_PROVIDER_BASE";}
  ${AUTH_ISSUER:+e.AUTH_ISSUER = "$AUTH_ISSUER";}
  ${AUTH_CLIENT_ID:+e.AUTH_CLIENT_ID = "$AUTH_CLIENT_ID";}
  ${AUTH_REDIRECT_PATH:+e.AUTH_REDIRECT_PATH = "$AUTH_REDIRECT_PATH";}
  ${AUTH_REDIRECT_URI:+e.AUTH_REDIRECT_URI = "$AUTH_REDIRECT_URI";}
  globalThis.__env = e;
})();
EOF

exec nginx -g "daemon off;"
