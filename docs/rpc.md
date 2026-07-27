# Gateway RPC

The CLI talks to a Quantova node through the gateway, a plain HTTP front door. Every request is a POST to a path of the form `/v1/<method>` with a JSON body, and every response is JSON. The CLI builds the bodies and parses the responses in the QCore core, so this page is a reference rather than something you assemble by hand.

## Transport

The base is the gateway url, from `--gateway` or `QTV_GATEWAY`. The native transport refuses to send a request in plain text to any host that is not loopback, since a fee or a nonce read over an open link would be unauthenticated. Run a gateway behind a trusted link before you point the CLI at a public host.

## Methods the CLI uses

Each command maps to one or a few gateway methods.

### node_info

Read by `qtv info`. Returns the chain id, the genesis hash, the head height, the denomination, the transfer fee, and the node version. The CLI reads it before signing so it can bind the chain id and check the fee.

### get_account

Read by `qtv account` and before every signed command. The body carries the address. Returns the address, the nonce, the balance, the scheme, and whether the key is registered.

### submit_transaction

Called by `qtv register`, `qtv send`, and the contract commands. The body carries the signed transaction bytes in hex. The verdict is accepted with a state and a transaction id, or rejected with a reason and, on a nonce mismatch, the expected and the given nonce.

### get_transaction

Read by `qtv tx`. The body carries the transaction id. The status is finalised with a height and a block, pending, or unknown.

### get_storage

Read by `qtv contract storage`. The body carries the contract address. Returns the storage slots, each a slot key and a whole number value.

### get_events

Read by `qtv events`. The body carries the block height. Returns the contract events in the block, each with the contract, the selector, and the data.

## Reading the wire yourself

Because a request is just a POST of JSON, you can reproduce any read with a normal HTTP client. For example a node info read is a POST of an empty object.

```
curl -s http://127.0.0.1:40404/v1/node_info -d '{}'
```

An account read carries the address.

```
curl -s http://127.0.0.1:40404/v1/get_account -d '{"address":"Q1..."}'
```

The full RPC surface, including the read methods an explorer uses, lives in the [QSP](https://github.com/Quantova/QSP) repository.
