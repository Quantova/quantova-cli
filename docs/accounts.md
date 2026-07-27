# Accounts and keys

A Quantova account is a post quantum signing key and the address derived from it. The CLI makes and reads accounts with the same core the whole client family uses, so an account made here is the same account a wallet or a script would make from the same seed.

## The signature scheme

Quantova signs with ML-DSA-65, the module lattice signature standardised by NIST as FIPS 204. It is the scheme identifier 1 that the CLI prints under `qtv key pubkey`. There is no classical fallback, so an account is quantum safe end to end.

## The seed and the phrase

An account starts from a thirty two byte seed. The CLI draws it from the platform random source under `qtv key new`. From the seed the CLI shows two things a developer keeps.

- The seed in hex, sixty four characters, the raw secret.
- A twenty four word recovery phrase, the only backup a human should write down.

The phrase encodes the same seed plus a checksum, so a single typo is caught on restore rather than resolving to a silent wrong account. Recover with `qtv key restore`.

```
qtv key new
qtv key restore "the twenty four words"
```

## Derivation and the index

One seed derives many accounts by index. Index 0 is the default. Pass `--index 1` for a second account under the same phrase. The address is deterministic, so the same seed and index always give the same address offline, with no gateway.

```
qtv key address <seed-or-phrase>
qtv key address <seed-or-phrase> --index 1
```

## The Q1 address format

A Quantova address is rendered in a Quantova bech32m form and begins with `Q1`. It is uppercase, and case does not change the signature, so a lowercased address signs the same transaction. The CLI checks an address is a valid Q1 address before it uses it, so a mistyped recipient is refused before anything is signed.

## Registering the key

An account must publish its public key once before it can send, so the chain can verify its signatures. Fund the address, then register.

```
qtv register
```

After that the account can sign transfers and contract calls. Read its state any time with `qtv account <address>`, which shows the balance, the nonce, the scheme, and whether the key is registered.
