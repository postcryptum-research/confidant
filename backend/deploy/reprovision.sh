#!/usr/bin/env bash
# Re-provision the Confidant demo pool after the public XRPL testnet has reset (the testnet periodically
# resets and wipes every account, so the pool, sink, treasury and issuer all vanish; the nodes' local
# state survives but every ledger read fails). This stands up a FRESH pool on the current testnet — it is
# a clean start, not a restore (the old bundles are gone with their accounts). Run ON THE HOST.
#
#   sudo bash backend/deploy/reprovision.sh
#
# What it does: back up data+env -> new pool (setup-pool.mjs: pool, sink, treasury, SignerList k-of-N,
# master disabled) -> new faucet issuer for the KYC gate -> install keys 0600 at the canonical paths ->
# clear node verifier state -> rewrite common.env (POOL/SINK/SIGNERS/treasury/issuer; screening, pof,
# quorum, rpc kept) -> restart services -> verify. Then seed an anonymity set with `npm run prepopulate`.
# Env overrides (defaults are the demo host's layout): REPO, DATA, ENV, SVCUSER, QUORUM, SIGNERS_N.
set -euo pipefail
REPO="${REPO:-/srv/confidant/repo}"; Q="$REPO/backend/quorum"
DATA="${DATA:-/srv/confidant/data/quorum}"; ENV="${ENV:-/srv/confidant/env}"
SVCUSER="${SVCUSER:-confidant}"; QUORUM="${QUORUM:-2}"; SIGNERS_N="${SIGNERS_N:-3}"
WS="${XRPL_WS:-wss://s.altnet.rippletest.net:51233}"; RPC="${XRPL_RPC:-https://s.altnet.rippletest.net:51234}"
KYC_HEX=436F6E666964616E744B5943   # "ConfidantKYC" — the credential TYPE is constant; only the issuer changes
TS=$(date -u +%Y%m%d-%H%M%S); BK="${BK:-/srv/confidant/reprovision-backup/$TS}"
NODE=$(command -v node)
asuser(){ sudo -u "$SVCUSER" bash -c "$1"; }
say(){ printf '\n== %s ==\n' "$*"; }

say "backup -> $BK"
asuser "mkdir -p '$BK' && cp -a '$DATA' '$BK/quorum' && cp -a '$ENV' '$BK/env'"

say "1. fresh pool (setup-pool: pool, sink, treasury, $QUORUM-of-$SIGNERS_N SignerList, master disabled)"
NEW="$DATA/new-$TS"; asuser "mkdir -p '$NEW'"
asuser "cd '$Q' && SIGNERS=$SIGNERS_N QUORUM=$QUORUM DIR='$NEW' FEE_SHIELD_BPS=25 FEE_UNSHIELD_BPS=25 XRPL_WS='$WS' $NODE setup-pool.mjs" > /tmp/reprov-setup.$TS.out 2>&1
POOL=$(asuser "$NODE -e \"console.log(require('$NEW/config.json').pool)\"")
SINK=$(asuser "$NODE -e \"console.log(require('$NEW/config.json').sink)\"")
TREAS=$(asuser "$NODE -e \"console.log(require('$NEW/config.json').treasury)\"")
SIGS=$(asuser "$NODE -e \"console.log(require('$NEW/config.json').signers.map(s=>s.address).join(','))\"")
echo "pool=$POOL sink=$SINK treasury=$TREAS"; echo "signers=$SIGS"

say "2. fresh faucet issuer for the KYC gate"
asuser "cd '$Q' && $NODE -e \"import('xrpl').then(async({Client})=>{const c=new Client('$WS');await c.connect();const{wallet:w}=await c.fundWallet();require('fs').writeFileSync('$NEW/issuer.key.json',JSON.stringify({address:w.address,seed:w.seed},null,2),{mode:0o600});console.log('ISSUER='+w.address);await c.disconnect();})\"" > /tmp/reprov-issuer.$TS.out 2>&1
ISSUER=$(grep -oE 'ISSUER=r[1-9A-HJ-NP-Za-km-z]+' /tmp/reprov-issuer.$TS.out | cut -d= -f2)
[ -n "$ISSUER" ] || { echo "FAILED: no issuer"; cat /tmp/reprov-issuer.$TS.out; exit 1; }
echo "issuer=$ISSUER"

say "3. install keys (0600) at the canonical paths + fresh config.json"
for f in config.json issuer.key.json treasury.key.json $(seq -f 'signer%g.key.json' 1 "$SIGNERS_N"); do asuser "cp '$NEW/$f' '$DATA/$f'"; done
asuser "cd '$DATA' && chmod 600 issuer.key.json treasury.key.json signer*.key.json"

say "4. clear node verifier state (fresh empty pool)"
asuser "rm -f '$DATA'/node*.verifier-state.json '$DATA'/node*.verifier-state.fees.json"

say "5. rewrite common.env (keep screening/pof/quorum/rpc)"
SCREEN=$(asuser "grep '^SCREENING_ARGS=' '$ENV/common.env' || echo 'SCREENING_ARGS='")
POF=$(asuser "grep '^POF_ARGS=' '$ENV/common.env' || echo 'POF_ARGS='")
asuser "cp '$ENV/common.env' '$ENV/common.env.bak-reprov-$TS'"
asuser "cat > '$ENV/common.env'" <<EOF
POOL=$POOL
SINK=$SINK
QUORUM=$QUORUM
SIGNERS=$SIGS
XRPL_RPC=$RPC
FEE_ARGS=--fee-shield-bps 25 --fee-unshield-bps 25 --fee-treasury $TREAS
$SCREEN
CREDENTIAL_ARGS=--credential-issuer $ISSUER --credential-type $KYC_HEX --credential-type-text ConfidantKYC
$POF
EOF

say "6. restart (nodes one at a time -> signers -> coordinator -> issuer -> wallet)"
for i in $(seq 0 "$SIGNERS_N"); do sudo systemctl restart "confidant-node@$i"; sleep 3; printf 'node@%s %s\n' "$i" "$(systemctl is-active confidant-node@$i)"; done
for i in $(seq 1 "$SIGNERS_N"); do sudo systemctl restart "confidant-signer@$i"; sleep 1; done
sudo systemctl restart confidant-coordinator; sleep 3
sudo systemctl restart confidant-issuer; sleep 2
sudo systemctl restart confidant-wallet; sleep 2
echo "failed units: $(systemctl --failed --no-legend | grep confidant || echo none)"

say "7. verify"
echo "fees: $(curl -s -m10 http://127.0.0.1:7900/fees | cut -c1-120)"
echo "pool on testnet: $(curl -s -m10 -X POST "$RPC" -H 'content-type: application/json' -d "{\"method\":\"account_info\",\"params\":[{\"account\":\"$POOL\",\"ledger_index\":\"validated\",\"signer_lists\":true}]}" | $NODE -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const r=JSON.parse(s).result;console.log(r.error||("EXISTS bal "+r.account_data.Balance+" flags "+r.account_data.Flags))})')"
echo
echo "DONE. pool=$POOL issuer=$ISSUER treasury=$TREAS sink=$SINK. rollback: $BK"
echo "Next: seed the anonymity set — cd $Q && COORDINATOR_URL=http://127.0.0.1:7900 CLI=$REPO/target/release/examples/cli ISSUER_URL=http://127.0.0.1:7960 npm run prepopulate"
