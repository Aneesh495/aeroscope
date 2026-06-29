# Aeroscope build and evaluation entry points.
#
# The resource-heavy targets (load, stress, acceptance) describe the full
# campaign honestly: they run a bounded pilot by default and print what the full
# declared profile requires, which this environment does not certify.

CARGO ?= cargo
RUN_ID ?= pilot-$(shell date +%Y%m%d-%H%M%S)
EVIDENCE ?= evidence/$(RUN_ID)

.PHONY: bootstrap doctor build test data demo bench stress docs acceptance verify release \
        ingest record replay load serve browser-test fmt clippy

bootstrap:
	$(CARGO) fetch
	@echo "bootstrap: Rust workspace fetched. For the web UI run 'npm install' in web/."

doctor:
	$(CARGO) run -q -p aero-server --bin aeroscope -- doctor

build:
	$(CARGO) build --workspace

test:
	$(CARGO) test --workspace

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace -- -D warnings

# A runnable end-to-end demonstration over synthetic replay.
demo:
	$(CARGO) run -q -p aero-server --bin aeroscope -- demo

# Ingest from a local readsb receiver (points at aircraft.json). Honest about
# requiring an actual receiver; without one, use `make demo`.
ingest:
	@echo "ingest: configure a readsb aircraft.json source, then run the server."
	@echo "        This target is a placeholder until the network transport lands."

# Record a bounded window of a source to the observation log.
record:
	@echo "record: writes an append-only observation log segment."
	@echo "        For a license-clean capture, use the synthetic generator."

# Replay a recorded log at variable speed (always labeled replay, never live).
replay:
	$(CARGO) run -q -p aero-server --bin aeroscope -- demo

# Synthetic load pilot. The full profile (50k tracks, 10k updates/s, 200 subs,
# 6 hours) needs a declared reference host and is not certified here.
load:
	$(CARGO) run -q --release -p aero-verify --bin aero-campaign -- load --pilot
	@echo "NOTE: full 6-hour / 50k-track profile is UNVERIFIED in this environment."

stress:
	$(CARGO) run -q --release -p aero-verify --bin aero-campaign -- stress --pilot

serve:
	@echo "serve: starts the HTTP + streaming server (transport layer pending)."

browser-test:
	@echo "browser-test: run the Playwright suite in web/ (see web/README)."

docs:
	@echo "docs: see docs/. Render diagrams with scripts/render-diagrams.sh."

# Run the frozen full campaign into a new evidence directory.
acceptance:
	$(CARGO) run -q --release -p aero-verify --bin aero-campaign -- acceptance \
		--run-id $(RUN_ID) --evidence $(EVIDENCE)

# Independently verify an existing evidence bundle.
verify:
	$(CARGO) run -q --release -p aero-verify --bin aero-verify -- --evidence $(EVIDENCE)

release:
	$(CARGO) build --workspace --release
	@echo "release: binaries in target/release. Verify remote parity before tagging."

data:
	@echo "data: pins airport/geographic datasets. See docs/DATA.md for provenance."
