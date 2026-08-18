# Contracts

Quantova runs contracts on the QVM, a register machine. A contract is written in the Quanta language, compiled to a container, deployed to an address, and called with encoded arguments. The CLI covers deploy, call, reading state, and reading an asset balance.

## The call ABI

A call is a four byte selector followed by a memory image. The node owns the first eighty eight bytes of that image, the call context, and overwrites them on every call. A caller can never set them, so a contract trusts them absolutely.

```
offset  width  meaning
0       32     caller, the address that signed the transaction
32      32     contract, this contract's own address
64      8      time, the block time in seconds
72      8      chain, the node's chain identity
80      8      value, the native Quon this call moved
88      ...    the entry arguments, laid out to the contract ABI
```

Arguments begin at offset eighty eight. Encode them at the offsets the [Quanta compiler](https://github.com/Quantova/Quanta-Smart-Contract-language) reports for the entry, or let a QCore client build the image, then pass the bytes as hex. The chain word stops an order signed for one chain from verifying on another. The value word is the payment the node actually moved, so a paid entry books exactly what changed hands and can never be told by attacker chosen calldata that it was paid more than it was.

## Deploy

A container is the compiled Quanta output. Deploy it from the signing account. The CLI prints the contract address, which is derived from the deployer and its nonce, so you know the address before the deploy even lands.

```
qtv contract deploy hello.qbc --key @deployer.key --max-fee 1000
```

A contract that reads `deploy_params` in its genesis takes those values on the command line, each typed so it lands at the width the genesis declared, in the order the genesis reads them.

```
qtv contract deploy Dex.qbc \
  addr:Q1OPERATOR... addr:Q1TOKENA... addr:Q1TOKENB... \
  --key @deployer.key --max-fee 1000
```

The types are `addr:<Q1>` for a thirty two byte address, `u64:<n>` for a word, `u128:<n>` for a wide value, and `guardians:<Q1,Q1,...>` for a guardian set. Deploy carries the whole container, so the CLI raises the execution meter above a bare transfer on its own. Set `--meter <n>` to override it when a large container needs more room.

## Call

A call sends a selector and its encoded arguments to a contract. The selector is the first four bytes, the rest are the arguments the entry expects at their ABI offsets. Build the bytes with the compiler ABI or a QCore client, then pass them as hex.

```
qtv contract call <address> 0a1b2c3d... --key @me.key --max-fee 1000 --meter 2000000
```

An entry that takes a payment reads the value the node injects at offset eighty. Move that payment with `--value <n>`, a whole number of Quon.

```
qtv contract call <qns-address> <register-args-hex> --value 1500 --key @me.key --max-fee 1000
```

The compiler binds a payable entry's asset argument to the value word, so the amount the contract books equals the value the node moved. A call whose argument claims a larger amount than `--value` carries is rejected and commits nothing. The CLI signs the call with your account key, submits it, and prints the verdict. A call that would emit an effect the node cannot settle is refused and commits nothing, so a failed call never leaves half state.

Orders that a third party signs, an operator quote or an owner authorised mint, are built and signed by the QCore clients, which lay the signature and the signed message into the argument region for the entry to verify. The CLI submits native transfers, deploys, and direct calls; an application that mints or quotes drives QCore directly.

## Read state

Contract storage is a set of slots, each a key and a whole number value. Read all of a contract's slots.

```
qtv contract storage <address>
```

Events are recorded per block. Read the contract events in a block by height. Each line prints the contract, the selector, and the event data in hex.

```
qtv events <height>
```

## Assets

An issuer's asset lives at the pair of the issuer and a holder, not inside contract storage. A contract moves its own holding of an asset to a holder with `send_asset`, and the node debits the contract and credits the holder, so a contract can only ever move what it holds. Read a holder's balance of an issuer's asset.

```
qtv asset balance <issuer> <holder>
```
