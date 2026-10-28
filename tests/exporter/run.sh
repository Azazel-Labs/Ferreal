#!/usr/bin/env bash
# Builds and runs the exporter over the made-up session (Fixture.cs), writing the reflection
# data the mock engine test reads. Needs the .NET SDK.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dotnet run --project "$here/exporter.csproj" -- "$here/../mock-ue/FerrealEngine.json"
