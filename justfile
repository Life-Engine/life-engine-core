# Life Engine — Development Commands

# Run the Core backend with auto-restart on source changes
dev-core:
    @echo "Starting Life Engine Core (cargo-watch)..."
    cargo watch -w apps/core/ -w packages/ -x 'run --bin life-engine-core'

# Run the Admin UI development server
dev-app:
    @echo "Starting Admin UI (vite)..."
    cd apps/admin && pnpm dev

# Run Core and Admin UI concurrently
dev-all:
    @echo "Starting Life Engine Core + Admin UI..."
    just dev-core & just dev-app & wait
