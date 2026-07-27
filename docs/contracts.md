# Contracts

Quantova runs contracts on the QVM, a register machine. A contract is written in the Quanta language, compiled to a container, deployed to an address, and called with encoded arguments. The CLI covers deploy, call, and reading state.

## Deploy

A container is the compiled Quanta output. Deploy it from the signing account. The CLI prints the contract address, which is derived from the deployer and its nonce, so you know the address before the deploy even lands.

```
qtv contract deploy hello.qbc --key @deployer.key
```

Deploy carries the whole container, so the CLI raises the execution meter above a bare transfer on its own. Set `--meter <n>` to override it when a large container needs more room.

## Call

A call sends a selector and its encoded arguments to a contract. The selector is the first four bytes of the call, and the rest are the arguments the entry expects, laid out to the contract ABI. Build the bytes with the Quanta compiler ABI or a QCore client, then pass them as hex.

```
qtv contract call <address> 0a1b2c3d0000000000000064 --meter 2000000
```

The CLI signs the call with your account key, submits it, and prints the verdict. Set `--meter` to the execution budget the call needs. A call that would emit an effect the node cannot settle is refused and commits nothing, so a failed call never leaves half state.

## Read state

Contract storage is a set of slots, each a key and a whole number value. Read all of a contract slots.

```
qtv contract storage <address>
```

Events are recorded per block. Read the contract events in a block by height. Each line prints the contract, the selector, and the event data in hex.

```
qtv events <height>
```

## The address a contract sees

Inside the machine a contract sees the leading bytes of a caller address as a handle, not the whole thirty two byte address. The node injects the trusted caller and the time at fixed offsets in every call, so a contract can trust who called it and when without a caller being able to forge either. When you need the full recipient for a value moving call, pass the full addresses in the call arguments and let the contract resolve the handle against them.
