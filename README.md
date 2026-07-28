# quantova-cli

The Quantova command line client. It gives a developer one terminal tool to make a post quantum account, read chain state, sign and submit transactions, and deploy and call contracts, over the Quantova gateway.

The binary is named `qtv`. It is a thin layer over [QCore.rs](https://github.com/Quantova/QCore.rs), the one Quantova client core, so the CLI never restates a signature or a wire format. Every key derivation, every ML-DSA-65 signature, and every gateway request comes from the same audited core that the JavaScript and Python clients are generated over.

## What it does

The CLI covers the whole path a developer walks on a fresh chain.

- Make an account offline, back it up as a twenty four word phrase, and derive its address.
- Read an account balance, nonce, and key state from a node.
- Register the account key so the account can sign.
- Sign and submit a native transfer, with a fee ceiling that refuses an inflated gateway fee.
- Read node info, follow a transaction to finality, and read contract storage and block events.
- Deploy a Quanta container and call a contract.

## Install

The CLI is part of the Quantova stack and builds on the QCore Rust core through a path dependency, the same way the core itself builds on the chain crates. Clone the stack repositories side by side, then build.

```
git clone https://github.com/Quantova/QCore.rs
git clone https://github.com/Quantova/quantova-cli
cd quantova-cli
cargo build --release
```

The binary lands at `target/release/qtv`. Put it on your path.

```
cp target/release/qtv ~/.local/bin/qtv
```

## Quick start

Point the CLI at a gateway once, then work against it. The gateway is a running Quantova node front door, for example the local devnet gateway.

```
export QTV_GATEWAY=http://127.0.0.1:40404

qtv info
qtv key new
```

`qtv key new` prints a seed, a phrase, and an address. Keep the seed and the phrase secret. Fund the address from a faucet or a funded account, then register the key and send.

```
export QTV_KEY="the twenty four word phrase you just saved"

qtv register
qtv account Q1U94GM6GYCCLREFF4HYDP04DKFD726WG3GTPRZW5A3UUC2YJ0373QLT9YNL
qtv send Q13ZKQW04ZHMVWNC5R6HR4ZSPE78X85KF5K5MK793PKHZXDRHS9D5Q3RZUPJ 1000000
qtv tx <the tx id printed above>
```

## Command reference

Run `qtv help` for the full list. Every command reads the gateway from `--gateway` or `QTV_GATEWAY` and the signing key from `--key` or `QTV_KEY`.

### key

Account and key work that runs offline. No gateway is contacted.

```
qtv key new                     create an account, print its seed, phrase, and address
qtv key address [<key>]         the address for a key and index
qtv key pubkey  [<key>]         the scheme, public key, and address, for a genesis file
qtv key restore <phrase>        recover a seed and address from a phrase
```

A `<key>` argument is a seed in hex, a twenty four word phrase in quotes, or `@path` to a file that holds either. When you leave it out the CLI reads `--key` or `QTV_KEY`.

### account and transactions

```
qtv account <address>           balance, nonce, scheme, and whether the key is registered
qtv register                    register the signing key so the account can send
qtv send <to> <amount>          sign and submit a native transfer
qtv info                        the chain id, genesis hash, height, fee, and version
qtv tx <tx-id>                  where a transaction is, pending, finalised, or unknown
```

`amount` is a whole number of base units. The fee is the gateway reported transfer fee. A signing command requires `--max-fee <n>`, your fee ceiling, and refuses to sign if the reported fee is above it, which stops an untrusted gateway from dictating an unbounded fee to drain a signer.

### contracts

```
qtv contract deploy <file>          deploy a Quanta container, print its address
qtv contract call <address> <hex>   call a contract with encoded arguments
qtv contract storage <address>      read a contract storage slots
qtv events <height>                 the contract events in a block
```

A container file is the compiled Quanta output. Encode call arguments with the [Quanta compiler](https://github.com/Quantova/Quanta-Smart-Contract-language) ABI or the QCore clients, then pass the bytes as hex. Deploy raises the execution meter above a bare transfer on its own. Set `--meter <n>` to override it.

## Flags and environment

```
-g, --gateway <url>   the gateway to talk to, or QTV_GATEWAY, default http://127.0.0.1:40404
-k, --key <value>     a seed hex, a phrase, or @file, or QTV_KEY
-i, --index <n>       the account index under one seed, default 0
    --max-fee <n>     refuse to sign if the gateway fee is above this
    --meter <n>       the execution meter for a contract call
```

One seed derives many accounts by index, so `--index 1` is a second account under the same backup phrase.

## Deeper reading

The `docs` folder carries the detail behind each area.

- [Accounts and keys](docs/accounts.md), the signature scheme, the Q1 address format, and derivation.
- [Transactions](docs/transactions.md), how a transfer is built, signed, and submitted, and the fee ceiling.
- [Contracts](docs/contracts.md), deploy and call a Quanta container and read its state.
- [Gateway RPC](docs/rpc.md), the wire the CLI speaks and every method it uses.
- [Configuration](docs/configuration.md), gateway and key resolution, environment, and key files.

## Where this sits in the stack

- [QCore.rs](https://github.com/Quantova/QCore.rs), the Rust client core the CLI is built on.
- [QCore.js](https://github.com/Quantova/QCore.js) and [QCore.py](https://github.com/Quantova/QCore.py), the JavaScript and Python clients over the same core.
- [Quanta-Smart-Contract-language](https://github.com/Quantova/Quanta-Smart-Contract-language), the contract language and compiler.
- [QVM](https://github.com/Quantova/QVM), the register machine that runs contracts.
- [Q-Primitives](https://github.com/Quantova/Q-Primitives) and [QSP](https://github.com/Quantova/QSP), the primitives and the RPC surface.

## License

Dual licensed under Apache-2.0 or MIT, at your option. Copyright Quantova Inc. See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and [NOTICE](NOTICE).
