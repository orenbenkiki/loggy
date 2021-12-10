#!/usr/bin/bash

# Execute a cargo command in a specific compilation configuration.
#
# Cargo has this delightful restriction that it only caches the built version of dependencies (and the current crate)
# for a specific compilation configurations. However, during development, one uses multiple configurations: one for
# `cargo check`, another for `cargo build` and `cargo test`, and yet another one for `cargo tarpaulin`. Each time one
# issues one of these commands, the results of compiling the dependencies and the crate itself using any other
# configuration is lost and the whole thing is rebuilt from scratch, which is needlessly slow.
#
# This script wraps around any cargo command and provides it with the cached copy of all the compiled results of the
# specific environment it wants. This still means one builds everything once per configuration but at least it is ONLY
# once.
#
# Typically you'd want at least a `base` configuration for `cargo build`, `cargo test` and `cargo doc`, a separate
# `check` configuration for `cargo check` and `cargo clippy`, and possibly a `tarpaulin` configuration for `cargo
# tarpaulin` coverage collection.
#
# This does mean that results will be stored in `.target.$NAME` instead of `target`, e.g. documentation will be
# generated into `.target.base/doc` instead of `target/doc`.
#
# This form of caching should really be a built-in feature of `cargo` itself.

NAME="$1"
shift

if [ -d target ]
then
    if [ -f target/NAME ]
    then
        OLD_NAME=`cat target/NAME`
        echo "Saving the abandoned target.$OLD_NAME" 1>&2
        mv target ".target.$OLD_NAME"
    else
        echo "Cowardly refusing to mess with an unnamed target directory" 1>&2
        exit 1
    fi
fi

if [ -f target ]
then
    rm -f target
fi

if [ -e ".target.$NAME" ]
then
    mv .target.$NAME target
else
    mkdir target
    echo "$NAME" > target/NAME
fi

function cleanup() {
    mv target .target.$NAME
    echo "Wrap all cargo commands with_configuration.sh!" > target
}

trap cleanup EXIT

"$@"
