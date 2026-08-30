fmt:
	cargo fmt --all

api-dev:
	cargo run -p projecty-api

web-dev:
	cd apps/web && npm run dev

web-check:
	cd apps/web && npm run check
