TOOLS = memo tap prv cel when

.PHONY: build test clippy install uninstall clean

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

clean:
	cargo clean
