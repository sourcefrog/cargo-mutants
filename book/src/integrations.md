# Integrations

## Shell completion

The `--completions SHELL` emits completion scripts for the given shell.

The right place to install these depends on your shell and operating system.

For example, for Fish[^fishconf]:

```sh
cargo mutants --completions fish >~/.config/fish/conf.d/cargo-mutants-completions.fish
```

[^fishconf]: This command installs them to `conf.d` instead of `completions` because you may have completions for several `cargo` plugins.

## vim-cargomutants

[`vim-cargomutants`](https://github.com/yining/vim-cargomutants) provides commands
view cargo-mutants results, see the diff of mutations, and to launch cargo-mutants
from within vim.
