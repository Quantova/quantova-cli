# Contributing

Thanks for helping on the Quantova command line client.

## Ground rules

The CLI is a thin layer over the QCore Rust core. Keep it that way. Key derivation, signing, transaction building, and the gateway wire belong in [QCore.rs](https://github.com/Quantova/QCore.rs), not here, so there is one implementation of anything that touches a user key or a signature.

## Before a pull request

- Build clean with `cargo build`.
- Keep comments to one line where a line earns its place, and out of the way where the code speaks for itself.
- Match the plain terminal output the commands already print, a label and a value per line.

## Reporting

Open an issue with the command you ran, the gateway you ran it against, and what you expected against what you saw. Never paste a seed or a recovery phrase into an issue.

## License

By contributing you agree your work is licensed under Apache-2.0 or MIT, at the project option, copyright Quantova Inc.
