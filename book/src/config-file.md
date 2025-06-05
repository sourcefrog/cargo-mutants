# Config file

Many options for cargo-mutants can be set in a config file. The config file is read from
`.cargo/mutants.toml` in the source tree root.

It's recommended that the config file be checked in to the source tree with values that will
allow developers to run `cargo mutants` with no other options.

`--no-config` can be used to disable reading the configuration file.

## Merging config and command-line options

When options are specified in both the config file and the command line, the command line options take precedence.

For options that take a list of values, values from the configuration file are appended
to values from the command line.

## Config file schema

A [JSON Schema](https://json-schema.org/) describes the fields in the config file and can be used
by many text editors to provide autocompletion and validation.

To generate the schema, run:

```bash
cargo mutants --emit-schema=config > config-schema.json
```
