# macOS Rust build troubleshooting

## Linker cannot read the default SDK

Rust dependencies with native build steps (including `aws-lc-sys`) can fail with:

```text
ld: tapi error: malformed file
MacOSX27.0.sdk/usr/lib/libSystem.B.tbd: error: unknown architecture
arm64e.x1-macos
```

This is an SDK/linker compatibility problem, not a Rust source error. An older
Apple linker/TAPI reader cannot parse the newer SDK's text-based library stubs.
The SDK chosen implicitly by `cc` can differ from the SDK reported by `xcrun`,
even when Xcode is the active developer directory.

### Diagnose

Run these commands separately to inspect the active developer tools and SDK:

```sh
xcode-select --print-path
xcrun --sdk macosx --show-sdk-path
cc --version
xcrun ld -v
ls /Library/Developer/CommandLineTools/SDKs
```

Also check for local `SDKROOT`, `DEVELOPER_DIR`, `CC`, `CXX`, Rust linker settings,
or `RUSTFLAGS` overrides. Do not publish environment files or credentials when
reporting diagnostics.

### Command-scoped workaround

If the SDK selected by `xcrun` is compatible with the installed linker, select it
explicitly for Cargo. Run from the repository root:

```sh
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" cargo test -p diraigent-api --lib
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" cargo clippy -p diraigent-api --all-targets -- -D warnings
```

If that SDK also fails, use an installed SDK known to be compatible. For example,
on the affected host, Command Line Tools SDK 26.5 works with Apple clang 17 and
ld 1230.1, while SDK 27.0 does not:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo test -p diraigent-api --lib
```

The path is an example, not a project requirement; verify that it exists locally.
The same command-scoped prefix applies to other Cargo build, test and Clippy
commands. Library tests avoid requiring PostgreSQL; integration tests may need a
configured database. Cargo normally detects the changed SDK environment for
native build scripts, so start by rerunning with the explicit SDK rather than
deleting the entire build cache.

### Permanent repair

Install matching Xcode/Command Line Tools and SDK versions, and select the intended
developer directory with `xcode-select` if needed. This is a machine-level change
for the host administrator; it can affect iOS builds and other projects. After
repair, verify a build without the `SDKROOT` prefix.

Do not pin a host-specific SDK path in tracked Cargo configuration, change Rust
dependencies to mask the mismatch, or commit local shell/deployment settings.
The workaround above changes only the environment of the specified command.
