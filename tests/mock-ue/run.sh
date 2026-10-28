#!/usr/bin/env bash
# Builds the example crate (which writes the C++ glue), compiles the example module's C++
# against the mock engine headers, links the Rust static library and runs the test.
# Usage: tests/mock-ue/run.sh [c++ compiler]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cxx="${1:-${CXX:-c++}}"
out="$root/target/mock-ue"
module="$root/examples/hello/Source/HelloRust"

# The engine's reflection data comes from the exporter's output for the made-up session of
# tests/exporter (as an Unreal build passes RivetEngine.json); the build refreshes the
# example's lock file from it.
RIVET_ENGINE_METADATA="$here/RivetEngine.json" \
	cargo build --manifest-path "$root/Cargo.toml" -p rivet-hello --quiet
lib="$root/target/debug/librivet_hello.a"

# Unreal Header Tool's generated headers, imitated: each defines GENERATED_BODY with the
# class' Super.
rm -rf "$out" && mkdir -p "$out/gen"
for header in $(find "$module/Public" -name '*.h'); do
	name="$(basename "$header" .h)"
	parent="$(grep -oE 'class [A-Z_]+ [A-Za-z0-9_]+ : public [A-Za-z0-9_]+' "$header" | awk '{print $NF}' | head -1 || true)"
	{
		echo "#undef GENERATED_BODY"
		if [ -n "$parent" ]; then
			echo "#define GENERATED_BODY() public: using Super = $parent; private:"
		else
			echo "#define GENERATED_BODY()"
		fi
	} > "$out/gen/$name.generated.h"
done

sources=$(find "$module/Private" -name '*.cpp')
"$cxx" -std=c++20 -Wall -Wextra -Wshadow -Wno-unused-parameter -Wno-unused-variable -Werror \
	-DHELLORUST_API= -I "$here/include" -I "$out/gen" -I "$module/Public" \
	$sources "$here/test_hello.cpp" "$lib" -lpthread -ldl -lm -o "$out/test_hello"
"$out/test_hello"
