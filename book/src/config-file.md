# Config file

Many options for cargo-mutants can be set in a config file. By default, the config is read from
`.cargo/mutants.toml` in the source tree root.

It's recommended that the config be checked in to the source tree with values that will
allow developers to run `cargo mutants` with no other options.

`--no-config` can be used to disable reading the configuration from the source tree.

`--config FILE` can be used to read configuration from a custom file instead of the default locations.
This is useful for having different configurations for different scenarios (e.g., CI/CD, development,
specific testing requirements).

## Where the config is read from

The config may be stored in any one of these locations, relative to the root of the source tree.
They are searched in this order and only the first one that exists is read: configuration from
several locations is never merged.

1. `.cargo/mutants.toml`
2. `mutants.toml`
3. `.mutants.toml`
4. `.config/mutants.toml`
5. The `[workspace.metadata.mutants]` table in `Cargo.toml`
6. The `[package.metadata.mutants]` table in `Cargo.toml`

The metadata tables hold the same keys as the config file, for example:

```toml
[workspace.metadata.mutants]
exclude_globs = ["src/generated/**/*.rs"]
timeout_multiplier = 2.0
```

For a full list of keys, see <https://github.com/sourcefrog/cargo-mutants/blob/main/src/config.rs>.

An example config file with detailed comments can be found at
[`examples/custom_config.toml`](https://github.com/sourcefrog/cargo-mutants/blob/main/examples/custom_config.toml).

## Merging config and command-line options

When options are specified in both the config file and the command line, for scalar options, the command line options take precedence.

For options that take a list of values, values from the configuration file are appended
to values from the command line.

## Config file schema

A [JSON Schema](https://json-schema.org/) describes the fields in the config file and can be used
by many text editors to provide autocompletion and validation.

A recent version of the schema is available at <https://json.schemastore.org/cargo-mutants-config.json>.

To generate the schema, run:

```bash
cargo mutants --emit-schema=config > config-schema.json
```
