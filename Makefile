ALL_SOURCES = $(shell git ls-files .)

RS_SOURCES = $(filter %.rs, $(ALL_SOURCES))

TOML_SOURCES = $(filter %.toml, $(ALL_SOURCES))

CARGO_SOURCES = $(RS_SOURCES) $(TOML_SOURCES)

TEST_FLAGS = RUST_BACKTRACE=1

define PRINT_HELP_PYSCRIPT
import re, sys

for line in sys.stdin:
	match = re.match(r'^([a-zA-Z_-]+):.*?## (.*)$$', line)
	if match:
		target, help = match.groups()
		print("%-20s %s" % (target, help.replace(' # ALLOW TODOX', '')))
endef
export PRINT_HELP_PYSCRIPT

help:  ## print this error message
	@python -c "$$PRINT_HELP_PYSCRIPT" < $(MAKEFILE_LIST)

ifeq ($(wildcard $(.no_todo_x)),)
todo_x:
else
todo_x: .make.todo_x  ## check for leftover TODOX # ALLOW TODOX
endif

TODO = todo$()x

.make.todo_x: $(TODO_X_SOURCES) # ALLOW TODOX
	cargo $(TODO)
	touch $@

formatted: .make.formatted  ## check code is properly formatted
	
.make.formatted: .cargo/config.toml $(CARGO_SOURCES)
	cargo fmt --all -- --check
	touch $@

reformat: .make.reformat  ## reformat the code
	
.make.reformat: .cargo/config.toml $(CARGO_SOURCES)
	cargo fmt --all
	touch $@

check: .make.check  ## check the sources

.make.check: .cargo/config.toml $(CARGO_SOURCES)
	cargo check --tests
	touch $@

smells: .make.smells  ## check for code smells with clippy
	
.make.smells: .cargo/config.toml .make.check
	cargo clippy -- --no-deps
	touch $@

build: .make.build  ## build everything

.make.build: .cargo/config.toml $(CARGO_SOURCES)
	cargo build
	cargo test --no-run
	touch $@

test: .make.test  ## run tests

.make.test: .cargo/config.toml .make.build
	$(TEST_FLAGS) cargo test -- --nocapture
	touch $@

retest: .cargo/config.toml  ## force re-run tests with nocapture
	$(TEST_FLAGS) cargo test -- --nocapture

coverage: .make.coverage  ## generate coverage report

.make.coverage: .make.test
	rm -f tarpaulin*
	$(TEST_FLAGS) cargo tarpaulin --skip-clean --out Xml
	touch $@

coverage-annotations: .make.coverage-annotations  ## check coverage annotations in code

.make.coverage-annotations: .cargo/config.toml .make.coverage
	cargo coverage-annotations
	touch $@

doc: .make.doc  ## generate documentation
	
.make.doc: .cargo/config.toml $(ALL_SOURCES)
	cargo doc --no-deps # --workspace
	touch $@

udeps: .make.udeps  ## check for unused dependencies
	
.make.udeps: .cargo/config.toml $(TOML_SOURCES) $(RS_SOURCES)
	cargo +nightly udeps --workspace --all-targets
	touch $@

outdated: .make.outdated  ## check all dependencies are up-to-date
	
.make.outdated: .cargo/config.toml $(TOML_SOURCES)
	cargo outdated --root-deps-only --exit-code 1
	touch $@

audit: .make.audit  ## audit dependencies for bugs or security issues
	
.make.audit: .cargo/config.toml $(TOML_SOURCES)
	cargo audit
	touch $@

common: todo_x formatted smells udeps coverage-annotations doc

dev: reformat tags common outdated audit  ## verify during development

staged:  ## check everything is staged for git commit
	@if git status . | grep -q 'Changes not staged\|Untracked files'; then git status; false; else true; fi

pre-commit: staged common outdated audit  ## verify everything before commit

pre-publish: .cargo/config.toml  ## publish dry run (post-commit)
	cargo publish --dry-run

on-push: common pre-publish  ## verify a pushed commit in a CI action

monthly: outdated audit  ## verify dependencies in a monthly CI action

publish: on-push monthly  ## actually publish
	cargo publish

tags: $(RS_SOURCES)  ## tags file for vim or Emacs.
	ctags --recurse .

clobber:  ## remove all generated files
	rm -f .make.* tags
	rm -rf .cargo target

clean:  ## remove generated files except for dependencies
	rm -f .make.* tags tarpaulin*
	rm -rf .cargo `find target -name '*clacks*'`

.cargo/config.toml:
	mkdir -p .cargo
	echo '[build]' > $@
	cargo tarpaulin --print-rust-flags | tail -1 | sed 's/RUSTFLAGS/rustflags/' >> $@