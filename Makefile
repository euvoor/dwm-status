watch:
	cargo watch \
		--clear \
		--quiet \
		--ignore my-target \
		--ignore target \
		--ignore Makefile \
		--shell "CARGO_TARGET_DIR=my-target cargo run --quiet"
