# every cargo invocation honours CARGO_BUILD_JOBS; set it to keep builds polite
# on shared machines, e.g. `CARGO_BUILD_JOBS=2 make test`.

# program keypair used for deploys. it lives outside the repo and is never committed.
PROGRAM_KEYPAIR ?= $(HOME)/.config/solana/nock-program-keypair.json
URL             ?= localnet
SO              := target/deploy/nock.so
ID_FILE         := interface/src/lib.rs
BUMP_FILE       := interface/src/constants.rs
# pinned verifiable-build image: agave 3.1.6, same as [workspace.metadata.cli]
VERIFY_IMAGE := solanafoundation/solana-verifiable-build:3.1.6@sha256:c48eb57342bd34543fb59a781d61e1b7f113bacc6af3ee3bb4026636627efbd6
REPO_URL ?= $(shell git remote get-url origin 2>/dev/null)
COMMIT   ?= $(shell git rev-parse HEAD 2>/dev/null)
RPC_URL  ?= https://api.mainnet-beta.solana.com

# the id compiled into the program (declare_id! in interface/src/lib.rs).
DECLARED_ID = $(shell sed -n 's/.*declare_id!("\([1-9A-HJ-NP-Za-km-z]*\)").*/\1/p' $(ID_FILE))

.PHONY: build test lint fmt client-test check-program-id deploy set-program-id \
	verify-build verify-hash verify-deployed clean

# compile the sbf program to target/deploy/nock.so
build:
	cargo build-sbf

# unit tests plus the mollusk tests, which load target/deploy/nock.so
test: build
	cargo test

# rustfmt and clippy as ci runs them
lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cd client && cargo fmt --check && cargo clippy --all-targets --features fetch -- -D warnings

fmt:
	cargo fmt --all
	cd client && cargo fmt

# the rust client, its own workspace: wire-format tests plus the fetch feature
client-test:
	cd client && cargo test && cargo check --features fetch

# refuses unless PROGRAM_KEYPAIR's pubkey is the declared ID. cargo build-sbf writes a
# random target/deploy/nock-keypair.json; deploying with that would put the program at an
# address whose PDAs it cannot sign for.
check-program-id:
	@test -f "$(PROGRAM_KEYPAIR)" || { echo "missing program keypair: $(PROGRAM_KEYPAIR)"; exit 1; }
	@actual=$$(solana-keygen pubkey "$(PROGRAM_KEYPAIR)") && \
	if [ "$$actual" != "$(DECLARED_ID)" ]; then \
		echo "refusing: $(PROGRAM_KEYPAIR) is $$actual, program declares $(DECLARED_ID)"; exit 1; \
	fi && echo "program id ok: $$actual"

deploy: check-program-id build
	solana program deploy --url $(URL) --program-id "$(PROGRAM_KEYPAIR)" $(SO)

# hook for a new program id: make set-program-id PROGRAM_KEYPAIR=path/to/new-keypair.json
# rewrites the id and the event authority (and its bump) everywhere they are pinned:
# declare_id!, the bump constant, the id test, idl/nock.json, the client and the README;
# then runs the tests that check them.
ID_PINNED := tests/src/constants.rs idl/nock.json client/src/generated/programs.rs \
	client/tests/wire_format.rs README.md
set-program-id:
	@test -f "$(PROGRAM_KEYPAIR)" || { echo "missing program keypair: $(PROGRAM_KEYPAIR)"; exit 1; }
	@old=$(DECLARED_ID) && new=$$(solana-keygen pubkey "$(PROGRAM_KEYPAIR)") && \
	pda() { solana find-program-derived-address $$1 string:event_authority --output json; } && \
	old_auth=$$(pda $$old | sed -n 's/.*"address": *"\([^"]*\)".*/\1/p') && \
	new_auth=$$(pda $$new | sed -n 's/.*"address": *"\([^"]*\)".*/\1/p') && \
	bump=$$(pda $$new | sed -n 's/.*"bumpSeed": *\([0-9]*\).*/\1/p') && \
	test -n "$$old_auth" && test -n "$$new_auth" && test -n "$$bump" && \
	for f in $(ID_FILE) $(ID_PINNED); do \
		sed -i.bak -e "s/$$old/$$new/g" -e "s/$$old_auth/$$new_auth/g" $$f && rm -f $$f.bak; \
	done && \
	sed -i.bak "s/^pub const EVENT_AUTHORITY_BUMP: u8 = [0-9]*;/pub const EVENT_AUTHORITY_BUMP: u8 = $$bump;/" $(BUMP_FILE) && \
	rm -f $(BUMP_FILE).bak && \
	echo "program id $$old -> $$new, event authority $$old_auth -> $$new_auth (bump $$bump)" && \
	cargo test -p nock-tests -- constants idl && cd client && cargo test

# reproducible build in docker; writes target/deploy/nock.so
verify-build:
	solana-verify build --library-name nock --base-image $(VERIFY_IMAGE)

# hash of the local build, comparable with `verify-deployed`
verify-hash:
	solana-verify get-executable-hash $(SO)

# rebuild COMMIT of REPO_URL in docker and compare with the program on RPC_URL
verify-deployed:
	solana-verify get-program-hash -u $(RPC_URL) $(DECLARED_ID)
	solana-verify verify-from-repo -u $(RPC_URL) --program-id $(DECLARED_ID) \
		--commit-hash $(COMMIT) --library-name nock --base-image $(VERIFY_IMAGE) \
		$(REPO_URL)

clean:
	cargo clean
	cd client && cargo clean
