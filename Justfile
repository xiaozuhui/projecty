fmt:
	cargo fmt --all

backend-dev:
	cargo run -p projecty-api

frontend-dev:
	cd frontend && npm run dev

frontend-check:
	cd frontend && npm run check
