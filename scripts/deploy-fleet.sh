#!/usr/bin/env bash
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$REPO/target/release/ytop"
MANIFEST="$HOME/.yggterm/apps/ytop.json"

if [ ! -x "$BIN" ]; then
    echo "Building ytop release binary..."
    cargo build --release --manifest-path "$REPO/Cargo.toml"
fi

BIN_MD5=$(md5sum "$BIN" | awk '{print $1}')
HOSTS="${1:-dev jojo oc practice jyas-webapp manin}"

echo "================================================================="
echo " Deploying ytop ($BIN_MD5) to fleet:"
echo " Hosts: $HOSTS"
echo "================================================================="

# Ensure local install
install "$BIN" "$HOME/.local/bin/ytop"
rm -f "$HOME/.local/bin/yggtopo" "$HOME/.yggterm/apps/yggtopo"*.json 2>/dev/null || true
echo "  ✅ local: installed ~/.local/bin/ytop"

for host in $HOSTS; do
    echo "--> Deploying to $host..."
    
    # 1. Push binary
    ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" \
        "mkdir -p ~/.local/bin ~/.yggterm/apps ~/.yggterm/bin"
    
    cat "$BIN" | ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" \
        "cat > ~/.local/bin/ytop.new && chmod 755 ~/.local/bin/ytop.new && mv -f ~/.local/bin/ytop.new ~/.local/bin/ytop"
    
    # 2. Push manifest
    if [ -f "$MANIFEST" ]; then
        cat "$MANIFEST" | ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" \
            "cat > ~/.yggterm/apps/ytop.json"
    fi

    # 3. Clean defunct yggtopo artifacts
    ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" \
        "rm -f ~/.local/bin/yggtopo ~/.yggterm/apps/yggtopo*.json 2>/dev/null || true"

    # 4. Verify MD5
    REMOTE_MD5=$(ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" "md5sum ~/.local/bin/ytop | awk '{print \$1}'")
    if [ "$REMOTE_MD5" = "$BIN_MD5" ]; then
        echo "  ✅ $host: ~/.local/bin/ytop ($REMOTE_MD5) matches release build"
    else
        echo "  ⛔ $host: MD5 MISMATCH! got $REMOTE_MD5, expected $BIN_MD5"
        exit 1
    fi
done

echo "================================================================="
echo " ytop fleet deployment complete & verified across all nodes."
echo "================================================================="
