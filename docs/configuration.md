# Configuration

The CLI is configured through two things, the gateway it talks to and the key it signs with. Each can come from a flag or the environment, so a developer sets it once and every command reads it.

## The gateway

The gateway is the node front door the CLI reads from and submits to. Resolution order.

1. The `--gateway <url>` flag, or its short form `-g`.
2. The `QTV_GATEWAY` environment variable.
3. The default `http://127.0.0.1:40404`, the local devnet gateway.

```
export QTV_GATEWAY=http://127.0.0.1:40404
qtv info

qtv info --gateway http://127.0.0.1:40404
```

## The key

The signing key is needed by any command that signs, which is register, send, and the contract deploy and call. Read only commands never need it. Resolution order.

1. The `--key <value>` flag, or its short form `-k`.
2. The `QTV_KEY` environment variable.

A key value takes three forms.

- A seed in hex, sixty four characters.
- A twenty four word recovery phrase, in quotes.
- `@path`, a file that holds either a seed or a phrase.

```
export QTV_KEY="the twenty four words"
qtv register

qtv send Q1... 1000000 --key @account.key
```

A key file keeps the secret out of your shell history and off the command line. Give it tight permissions.

```
umask 077
qtv key new | sed -n 's/^phrase  //p' > account.key
```

## The index

One seed derives many accounts. The `--index <n>` flag, short form `-i`, picks the account under the seed, default 0. The same seed and index always give the same address.

## The fee ceiling and the meter

`--max-fee <n>` is the most you will pay in fee. If the gateway reports a higher fee the CLI refuses to sign. `--meter <n>` sets the execution budget for a contract call. Deploy raises the meter on its own when you do not set one.

## A minimal setup

A working shell for a testnet session.

```
export QTV_GATEWAY=http://127.0.0.1:40404
export QTV_KEY="your twenty four word phrase"

qtv info
qtv account "$(qtv key address)"
qtv send Q1... 1000000 --max-fee 1000
```
