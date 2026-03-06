TOOLS = memo tap prv cel when

.PHONY: build test clippy install uninstall clean test-linux

build:
	cargo build --workspace --release

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace

install: build
	@for tool in $(TOOLS); do \
		cp target/release/$$tool $${CARGO_HOME:-$$HOME/.cargo}/bin/$$tool; \
		echo "installed $$tool"; \
	done

uninstall:
	@for tool in $(TOOLS); do \
		rm -f $${CARGO_HOME:-$$HOME/.cargo}/bin/$$tool; \
		echo "removed $$tool"; \
	done

test-linux:
	docker run --rm \
		-v $(CURDIR):/workspace \
		-w /workspace \
		rust:1-slim \
		sh -c "apt-get update -qq && apt-get install -y -qq bsdutils >/dev/null 2>&1 && cargo test --workspace"

clean:
	cargo clean
