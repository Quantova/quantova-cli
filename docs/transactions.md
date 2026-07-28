# Transactions

Every transaction the CLI sends is built and signed inside the QCore core and submitted to the gateway. The terminal never assembles the wire by hand.

## How a transfer is built

A native transfer is a call to the recipient address that carries the amount encoded in the body. The core encodes the amount, sets the transfer meter, binds the sender nonce and the chain id, and signs the whole body with the account key. The result is a signed transaction and its id.

The steps the CLI runs for `qtv send`.

1. Read the node info to learn the chain id and the current transfer fee.
2. Refuse to sign if the fee is above the ceiling you set with `--max-fee`.
3. Read the sender account to learn its next nonce.
4. Sign the transfer with the account key, the nonce, the fee, and the chain id.
5. Submit the signed bytes and print the verdict.

```
qtv send <to> <amount> --max-fee 500
```

## The fee ceiling

The gateway reports the fee, and the CLI signs at that fee. Because a gateway is not always trusted, `--max-fee` sets the most you will pay. If the gateway asks for more the CLI refuses to sign and returns an error, so an inflated fee can never drain a signer. A signing command requires `--max-fee`, and refuses to sign without it, so a gateway can never dictate an unbounded fee.

## Nonces

Each account carries a nonce that counts its sent transactions. The CLI reads the current nonce from the node before it signs, so a transfer lands with the right nonce. If the node reports a mismatch the verdict carries the expected and the given nonce, which the CLI prints on a rejection.

## Following a transaction

Submit returns a transaction id. Follow it to finality.

```
qtv tx <tx-id>
```

The status is pending while the transaction waits, finalised once it lands in a finalised block, at the height and block the command prints, or unknown if the node has not seen it.

## The chain id

The chain id is folded into the signed body, so a transaction signed for one network cannot replay on another. The CLI reads the chain id from the node info and passes it into the signature, so a developer never sets it by hand.
