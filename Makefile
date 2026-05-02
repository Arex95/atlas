SHELL := bash
BINARY := target/release/atlas-server
WEB_DIST := apps/web/dist

.PHONY: install build-web build-server start dev clean check

## Full install: build frontend + compile server (run once after cloning)
install: build-web build-server
	@echo ""
	@echo "  Atlas is ready."
	@echo "  Run:  make start"
	@echo ""

## Build the Vue frontend into apps/web/dist/
build-web:
	@echo ">>> Building frontend..."
	pnpm --filter web build

## Compile the Rust server (release mode)
build-server:
	@echo ">>> Compiling server..."
	cargo build --release -p atlas-server

## Start the production server (frontend + API on :4000)
start:
	@echo ">>> Starting Atlas on http://localhost:4000"
	./$(BINARY)

## Dev mode: hot-reload server + Vite dev server in parallel
dev:
	@trap 'kill 0' SIGINT; \
	VITE_SERVER_URL=http://localhost:4000 pnpm --filter web dev & \
	cargo run -p atlas-server & \
	wait

## Zero-warning check (CI gate)
check:
	RUSTFLAGS="-D warnings" cargo build -p atlas-server
	cargo clippy -p atlas-server -- -D warnings
	pnpm --filter web build

## Remove build artifacts
clean:
	cargo clean
	rm -rf $(WEB_DIST)
