#!/bin/sh
# Creates the self-signed code-signing identity that scripts/build-dev-app.sh signs with.
# macOS keys an Accessibility grant to the app's designated requirement. An ad-hoc build's
# requirement is its cdhash, which changes on every rebuild and silently revokes the grant.
# A stable identity makes the requirement "identifier + certificate", which survives rebuilds.
set -eu
# macOS ships LibreSSL here; its PKCS#12 defaults are ones `security import` accepts, unlike OpenSSL 3.
openssl=/usr/bin/openssl
identity="konnyaku Local Dev"
keychain=${KONNYAKU_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}

if security find-certificate -c "$identity" "$keychain" >/dev/null 2>&1; then
  echo "\"$identity\" already exists in $keychain"
  exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat >"$tmp/req.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $identity
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
EOF
"$openssl" req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$tmp/req.cnf" \
  -keyout "$tmp/key.pem" -out "$tmp/cert.pem"
password=$("$openssl" rand -hex 16)
"$openssl" pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" \
  -out "$tmp/identity.p12" -passout "pass:$password"
security import "$tmp/identity.p12" -k "$keychain" -P "$password" -T /usr/bin/codesign
echo "created \"$identity\" in $keychain"
