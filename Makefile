ALL_SOURCES = $(shell git ls-files .)

RS_SOURCES = $(filter %.rs, $(ALL_SOURCES))

TOML_SOURCES = $(filter %.toml, $(ALL_SOURCES))

CARGO_SOURCES = $(RS_SOURCES) $(TOML_SOURCES)

define PRINT_HELP_PYSCRIPT
import re, sys

for line in sys.stdin:
	match = re.match(r'^([a-zA-Z_-]+):.*?## (.*)$$', line)
	if match:
		target, help = match.groups()
		print("%-20s %s" % (target, help.replace('TODO-', 'TODO')))
endef
export PRINT_HELP_PYSCRIPT

help:
	@python -c "$$PRINT_HELP_PYSCRIPT" < $(MAKEFILE_LIST)

TAGS: $(RS_SOURCES)  ## TAGS file for vim or Emacs.
	rust-ctags .

TEST_FLAGS = RUST_TEST_THREADS=1 RUST_BACKTRACE=1

.cargo/config.toml:
	mkdir -p .cargo
	echo '[build]' > $@
	cargo tarpaulin --print-rust-flags | tail -1 | sed 's/RUSTFLAGS/rustflags/' >> $@

retest:  ## force re-run tests
	$(TEST_FLAGS) cargo test -- --nocapture

test: .make.test  ## run tests

.make.test: .cargo/config.toml $(CARGO_SOURCES)
	$(TEST_FLAGS) cargo test -- --nocapture
	touch $@

check: .make.check  ## check the sources

.make.check: .cargo/config.toml $(CARGO_SOURCES)
	cargo check --tests
	touch $@

build: .make.build  ## build the binaries

.make.build: .cargo/config.toml $(CARGO_SOURCES)
	cargo test --no-run
	touch $@

pc: staged verify  ## check everything before commit

ci: verify pre-publish ## check everything in a CI server

verify: fmt clippy test coverage-annotations doc outdated audit

fmt: .make.fmt  ## check code format
	
.make.fmt: .cargo/config.toml $(CARGO_SOURCES)
	cargo fmt -- --check
	touch $@

refmt: .make.refmt  ## reformat code
	
.make.refmt: .cargo/config.toml $(CARGO_SOURCES)
	cargo fmt
	touch $@

staged:  ## check everything is staged for git commit
	@if git status . | grep -q 'Changes not staged\|Untracked files'; then git status; false; else true; fi

outdated: .make.outdated  ## check all dependencies are up-to-date
	
.make.outdated: .cargo/config.toml $(TOML_SOURCES)
	cargo outdated --root-deps-only --exit-code 1
	touch $@

clippy: .make.clippy  ## check for code smells with clippy
	
.make.clippy: .make.check
	cargo clippy -- --no-deps
	touch $@

doc: .make.doc  ## generate documentation
	
.make.doc: .cargo/config.toml $(ALL_SOURCES)
	cargo doc --no-deps
	touch $@

coverage: .make.coverage  ## generate coverage report

.make.coverage: .cargo/config.toml $(CARGO_SOURCES)
	mv .cargo/config.toml .cargo/_config.toml
	$(TEST_FLAGS) cargo tarpaulin --skip-clean --out Xml
	mv .cargo/_config.toml .cargo/config.toml
	touch $@

coverage-annotations: .make.coverage-annotations  ## check coverage annotations in code

.make.coverage-annotations: .make.coverage
	cargo coverage-annotations
	touch $@

audit: .make.audit  ## audit dependencies for bugs or security issues
	
.make.audit: .cargo/config.toml $(TOML_SOURCES)
	cargo audit
	touch $@

clean:  ## remove all build, test, coverage and Python artifacts
	rm -f .make.*
	rm -rf .cargo target

pre-publish: .cargo/config.toml  ## pre-publish check
	cargo publish --dry-run

publish: ci  ## actually publish
	cargo publish
