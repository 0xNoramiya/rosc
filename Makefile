# Code Blue ($ROSC)
.PHONY: test build sim charts web test-web

# Rules, properties, the defaults table and the simulator, then the contract
# on a local sandbox chain. The contract tests need cargo-near on PATH and
# run few at a time: every test starts its own sandbox.
test:
	cargo test --workspace --release
	cd contracts/rosc && cargo test -- --test-threads=3

# The contract wasm: contracts/rosc/target/near/rosc.wasm
build:
	cd contracts/rosc && cargo near build non-reproducible-wasm

# Six scenarios x 1,000 runs for the live preset, the pair timing, the sweep, then the charts.
sim:
	cargo run --release -p codeblue-sim -- all
	cargo run --release -p codeblue-sim -- run --preset live-24h
	cargo run --release -p codeblue-sim -- run --preset live-6h
	node sim/render-charts.mjs

charts:
	node sim/render-charts.mjs

# ── Testnet: bots, pager and the demo ──────────────────────────────────
# Keys stay in bots/.keys/ (git-ignored). Never mainnet.
.PHONY: demo deploy-demo pager holders pacer rescue

bots/node_modules: bots/package.json
	cd bots && npm install --no-audit --no-fund
	@touch bots/node_modules

# Launches a demo-preset coin on testnet and leaves it without access keys.
deploy-demo: bots/node_modules
	@test -f contracts/rosc/target/near/rosc.wasm || $(MAKE) build
	node bots/deploy.mjs testnet-demo

# Plays the whole demo script on testnet and prints a link for every transaction.
# HUMAN=1 to do the CPR yourself on the page; FRESH=1 to deploy new coins.
demo: bots/node_modules
	@test -f contracts/rosc/target/near/rosc.wasm || $(MAKE) build
	node bots/demo.mjs

# Posts arrests, saves and deaths to PAGER_WEBHOOK, or prints them.
pager: bots/node_modules
	node bots/pager.mjs testnet-demo

# The stand-in community, one bot per terminal.
holders: bots/node_modules
	node bots/holders.mjs testnet-demo
pacer: bots/node_modules
	node bots/pacer.mjs testnet-demo
rescue: bots/node_modules
	node bots/rescue.mjs testnet-demo

# The bedside monitor, reading the coins in deployments/.
web: web/node_modules
	cd web && npm run dev

# The page's acceptance tests, in a real browser against testnet. The full
# code from the keyboard needs a coin that is about to arrest:
#   node bots/deploy.mjs testnet-page-test && CODE_COIN=testnet-page-test make test-web
test-web: web/node_modules
	cd web && npx playwright test

web/node_modules:
	cd web && npm install
