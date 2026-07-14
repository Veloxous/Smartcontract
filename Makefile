.PHONY: build test deploy-testnet

build:
	stellar contract build

test:
	cargo test

deploy-testnet: build
	@mkdir -p deploy
	@ESCROW_ID=$$(stellar contract deploy \
	  --wasm target/wasm32v1-none/release/veloxous_escrow.wasm \
	  --source $(STELLAR_SECRET_KEY) \
	  --network testnet \
	  -- \
	  --admin $(ADMIN_ADDRESS)) && \
	echo "VeloxousEscrow: $$ESCROW_ID" && \
	printf '{"network":"testnet","veloxous_escrow":"%s"}\n' \
	  "$$ESCROW_ID" > deploy/testnet.json && \
	echo "Saved to deploy/testnet.json"
