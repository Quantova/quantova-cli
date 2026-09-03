// Copyright 2026 Quantova Inc
// SPDX-License-Identifier: Apache-2.0 OR MIT

// qtv, the Quantova command line client. It reuses the qcore Rust core for key
// derivation, ML-DSA-65 signing, transaction building, and the gateway wire, so
// the terminal never restates a signature or a request the machine would reject.

use qcore::{
    account_address, account_public_key, address_payload,
    contract::{DeployParam, FieldArg, FieldValue, DEFAULT_REGION_OFFSET},
    generate_seed, mnemonic_from_seed, seed_from_mnemonic, valid_address, Client, Submit, TxStatus,
};
use qtv_wipe::Zeroizing;

const DEFAULT_GATEWAY: &str = "http://127.0.0.1:40404";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run(&args) {
        eprintln!("error {message}");
        std::process::exit(1);
    }
}

// the flags every command may carry, pulled out of the argument list so the rest are positionals
struct Flags {
    gateway: String,
    key: Option<Zeroizing<String>>,
    key_on_argv: bool,
    index: u64,
    max_fee: u128,
    max_fee_set: bool,
    meter: u64,
    value: u64,
    asset: Option<String>,
    scheme_off: Option<u64>,
    ptr_off: Option<u64>,
    fields: Vec<String>,
}

fn run(args: &[String]) -> Result<(), String> {
    let command = args.first().map(String::as_str).unwrap_or("help");
    if matches!(command, "help" | "-h" | "--help") {
        print_usage();
        return Ok(());
    }
    if matches!(command, "version" | "-V" | "--version") {
        println!("qtv {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (flags, rest) = parse_flags(&args[1..])?;
    match command {
        "key" => cmd_key(&rest, &flags),
        "account" => cmd_account(&rest, &flags),
        "register" => cmd_register(&flags),
        "send" => cmd_send(&rest, &flags),
        "info" => cmd_info(&flags),
        "tx" => cmd_tx(&rest, &flags),
        "contract" => cmd_contract(&rest, &flags),
        "asset" => cmd_asset(&rest, &flags),
        "events" => cmd_events(&rest, &flags),
        other => {
            print_usage();
            Err(format!("unknown command '{other}'"))
        }
    }
}

fn parse_flags(args: &[String]) -> Result<(Flags, Vec<String>), String> {
    let mut flags = Flags {
        gateway: std::env::var("QTV_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.to_string()),
        key: std::env::var("QTV_KEY").ok().map(Zeroizing::new),
        key_on_argv: false,
        index: 0,
        // Fail closed: with no ceiling set, a signing command refuses rather than accepting whatever
        // fee the gateway dictates. A gateway that reports the account balance as the fee would drain
        // it, so the ceiling is required before anything is signed.
        max_fee: 0,
        max_fee_set: false,
        meter: qcore::NATIVE_TRANSFER_METER,
        value: 0,
        asset: None,
        scheme_off: None,
        ptr_off: None,
        fields: Vec::new(),
    };
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let mut value = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--gateway" | "-g" => flags.gateway = value("--gateway")?,
            "--key" | "-k" => {
                flags.key = Some(Zeroizing::new(value("--key")?));
                flags.key_on_argv = true;
            }
            "--index" | "-i" => {
                flags.index = value("--index")?
                    .parse()
                    .map_err(|_| "the index is not a number")?
            }
            "--max-fee" => {
                flags.max_fee = value("--max-fee")?
                    .parse()
                    .map_err(|_| "the max fee is not a number")?;
                flags.max_fee_set = true;
            }
            "--meter" => {
                flags.meter = value("--meter")?
                    .parse()
                    .map_err(|_| "the meter is not a number")?
            }
            "--value" => {
                flags.value = value("--value")?
                    .parse()
                    .map_err(|_| "the value is not a number")?
            }
            "--asset" => flags.asset = Some(value("--asset")?),
            "--scheme-off" => {
                flags.scheme_off = Some(
                    value("--scheme-off")?
                        .parse()
                        .map_err(|_| "the scheme offset is not a number")?,
                )
            }
            "--ptr-off" => {
                flags.ptr_off = Some(
                    value("--ptr-off")?
                        .parse()
                        .map_err(|_| "the pointer offset is not a number")?,
                )
            }
            "--field" => flags.fields.push(value("--field")?),
            _ => rest.push(arg.clone()),
        }
        i += 1;
    }
    Ok((flags, rest))
}

// the fee ceiling a signing command will not exceed. It has no default: a command that signs must be
// told the most it may pay, so an untrusted gateway can never dictate an unbounded fee and drain the
// account. A read only command never calls this.
fn require_max_fee(flags: &Flags) -> Result<u128, String> {
    if flags.max_fee_set {
        Ok(flags.max_fee)
    } else {
        Err(
            "pass --max-fee <n>, the most Quon you will let the gateway charge in fee for this \
             transaction, so an untrusted gateway cannot inflate the fee to drain the account"
                .to_string(),
        )
    }
}

fn warn_key_on_argv(raw: &str) {
    if !raw.trim_start().starts_with('@') {
        eprintln!(
            "warning: a key on the command line is visible to other users through the process list \
             and shell history; prefer @file or the QTV_KEY environment variable"
        );
    }
}

// a key is a sixty four character seed in hex, a twenty four word recovery phrase, or @path to a file holding either
fn resolve_key(flags: &Flags) -> Result<Zeroizing<[u8; 32]>, String> {
    let raw = flags
        .key
        .clone()
        .ok_or("no key given, pass --key <seed-or-phrase> or set QTV_KEY")?;
    if flags.key_on_argv {
        warn_key_on_argv(&raw);
    }
    parse_key_value(&raw)
}

fn refuse_if_key_file_is_shared(path: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(format!(
                    "the key file {path} is readable by group or others, restrict it with chmod 600 before use"
                ));
            }
        }
    }
    let _ = path;
    Ok(())
}

fn parse_key_value(raw: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let raw = raw.trim();
    if let Some(path) = raw.strip_prefix('@') {
        refuse_if_key_file_is_shared(path)?;
        let body = Zeroizing::new(
            std::fs::read_to_string(path).map_err(|e| format!("read the key file: {e}"))?,
        );
        return parse_key_value(&body);
    }
    if raw.split_whitespace().count() >= 2 {
        return seed_from_mnemonic(raw);
    }
    parse_seed_hex(raw)
}

fn parse_seed_hex(hex: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err(
            "a seed is sixty four hex characters, or pass a twenty four word phrase".to_string(),
        );
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    for (i, pair) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| "the seed is not hex")?;
        seed[i] = u8::from_str_radix(text, 16).map_err(|_| "the seed is not hex")?;
    }
    Ok(seed)
}

fn cmd_key(args: &[String], flags: &Flags) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("") {
        "new" => {
            let seed = generate_seed()?;
            let seed_hex = Zeroizing::new(to_hex(&seed[..]));
            let phrase = Zeroizing::new(mnemonic_from_seed(&seed));
            println!("seed    {}", seed_hex.as_str());
            println!("phrase  {}", phrase.as_str());
            println!("address {}", account_address(&seed, flags.index));
            println!();
            println!(
                "Keep the seed and the phrase secret. The phrase is the only backup of this key."
            );
            Ok(())
        }
        "address" => {
            let seed = key_from_arg_or_flag(args.get(1), flags)?;
            println!("{}", account_address(&seed, flags.index));
            Ok(())
        }
        "pubkey" => {
            let seed = key_from_arg_or_flag(args.get(1), flags)?;
            println!("scheme  1");
            println!(
                "pubkey  {}",
                to_hex(&account_public_key(&seed, flags.index))
            );
            println!("address {}", account_address(&seed, flags.index));
            Ok(())
        }
        "restore" => {
            let phrase = Zeroizing::new(args[1..].join(" "));
            if phrase.trim().is_empty() {
                return Err("usage: qtv key restore <twenty four word phrase>".to_string());
            }
            warn_key_on_argv(&phrase);
            let seed = seed_from_mnemonic(&phrase)?;
            let seed_hex = Zeroizing::new(to_hex(&seed[..]));
            println!("seed    {}", seed_hex.as_str());
            println!("address {}", account_address(&seed, flags.index));
            Ok(())
        }
        _ => Err("usage: qtv key <new | address | pubkey | restore>".to_string()),
    }
}

fn key_from_arg_or_flag(
    arg: Option<&String>,
    flags: &Flags,
) -> Result<Zeroizing<[u8; 32]>, String> {
    match arg {
        Some(value) => {
            warn_key_on_argv(value);
            parse_key_value(value)
        }
        None => resolve_key(flags),
    }
}

fn cmd_account(args: &[String], flags: &Flags) -> Result<(), String> {
    let address = args.first().ok_or("usage: qtv account <address>")?;
    if !valid_address(address) {
        return Err("the address is not a Q1 address".to_string());
    }
    let account = Client::new(flags.gateway.clone()).account(address)?;
    println!("address {}", account.address);
    println!("balance {}", account.balance);
    println!("nonce   {}", account.nonce);
    println!("scheme  {}", account.scheme);
    println!("has key {}", account.has_key);
    Ok(())
}

fn cmd_register(flags: &Flags) -> Result<(), String> {
    let seed = resolve_key(flags)?;
    let max_fee = require_max_fee(flags)?;
    let (_signed, outcome) =
        Client::new(flags.gateway.clone()).register(&seed, flags.index, max_fee)?;
    report_submit("registered", outcome)
}

fn cmd_send(args: &[String], flags: &Flags) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: qtv send <to> <amount> --key <seed-or-phrase>".to_string());
    }
    let to = &args[0];
    let amount: u64 = args[1].parse().map_err(|_| "the amount is not a number")?;
    let seed = resolve_key(flags)?;
    let max_fee = require_max_fee(flags)?;
    let (_signed, outcome) =
        Client::new(flags.gateway.clone()).transfer(&seed, flags.index, to, amount, max_fee)?;
    report_submit("submitted", outcome)
}

fn cmd_info(flags: &Flags) -> Result<(), String> {
    let info = Client::new(flags.gateway.clone()).node_info()?;
    println!("chain   {}", info.chain_id);
    println!("genesis {}", info.genesis_hash);
    println!("height  {}", info.head_height);
    println!("fee     {} {}", info.transfer_fee, info.denomination);
    println!("version {}", info.version);
    Ok(())
}

fn cmd_tx(args: &[String], flags: &Flags) -> Result<(), String> {
    let tx_id = args.first().ok_or("usage: qtv tx <tx-id>")?;
    match Client::new(flags.gateway.clone()).transaction(tx_id)? {
        TxStatus::Finalised { height, block } => {
            println!("finalised at height {height} in block {block}")
        }
        TxStatus::Pending => println!("pending"),
        TxStatus::Unknown => println!("unknown"),
    }
    Ok(())
}

fn cmd_contract(args: &[String], flags: &Flags) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("") {
        "deploy" => {
            let path = args.get(1).ok_or(
                "usage: qtv contract deploy <container-file> [param ...]\n\
                 a param is typed as addr:<Q1>, u64:<n>, u128:<n>, or guardians:<Q1,Q1,...>, \
                 given in the order the contract's genesis reads deploy_params",
            )?;
            let container = std::fs::read(path).map_err(|e| format!("read the container: {e}"))?;
            let params = parse_deploy_params(&args[2..])?;
            let seed = resolve_key(flags)?;
            let max_fee = require_max_fee(flags)?;
            let meter = deploy_meter(flags);
            let (_signed, outcome, address) = Client::new(flags.gateway.clone())
                .deploy_with_params(&seed, flags.index, &container, &params, meter, max_fee)?;
            println!("contract {address}");
            report_submit("deployed", outcome)
        }
        "call" => {
            if args.len() < 3 {
                return Err(
                    "usage: qtv contract call <address> <args-hex> [--value <n>]".to_string(),
                );
            }
            let target = &args[1];
            let call_args = from_hex(&args[2])?;
            let seed = resolve_key(flags)?;
            let max_fee = require_max_fee(flags)?;
            let client = Client::new(flags.gateway.clone());
            let (_signed, outcome) = match &flags.asset {
                Some(issuer) => client.call_asset(
                    &seed,
                    flags.index,
                    target,
                    call_args,
                    issuer,
                    flags.value,
                    call_meter(flags),
                    max_fee,
                )?,
                None => client.call_payable(
                    &seed,
                    flags.index,
                    target,
                    call_args,
                    flags.value,
                    call_meter(flags),
                    max_fee,
                )?,
            };
            report_submit("called", outcome)
        }
        "order" => {
            if args.len() < 3 {
                return Err(
                    "usage: qtv contract order <address> <selector-hex> --scheme-off <n> \
                            --ptr-off <n> --field <offset:type:value> ... --key <owner>"
                        .to_string(),
                );
            }
            let target = &args[1];
            let selector = parse_selector(&args[2])?;
            let scheme_off = flags
                .scheme_off
                .ok_or("pass --scheme-off <n>, the order scheme word offset")?;
            let ptr_off = flags
                .ptr_off
                .ok_or("pass --ptr-off <n>, the order pointer word offset")?;
            let fields = parse_order_fields(&flags.fields)?;
            let seed = resolve_key(flags)?;
            let max_fee = require_max_fee(flags)?;
            let (_signed, outcome, _order) = Client::new(flags.gateway.clone()).call_typed_order(
                &seed,
                flags.index,
                target,
                selector,
                scheme_off,
                ptr_off,
                DEFAULT_REGION_OFFSET,
                &fields,
                &seed,
                flags.index,
                flags.value,
                flags.asset.as_deref(),
                call_meter(flags),
                max_fee,
            )?;
            report_submit("ordered", outcome)
        }
        "storage" => {
            let address = args.get(1).ok_or("usage: qtv contract storage <address>")?;
            let slots = Client::new(flags.gateway.clone()).storage(address)?;
            println!("slots {}", slots.len());
            for slot in slots {
                println!("  {} {}", to_hex(&slot.slot), slot.value);
            }
            Ok(())
        }
        _ => Err("usage: qtv contract <deploy | call | order | storage>".to_string()),
    }
}

fn parse_deploy_params(args: &[String]) -> Result<Vec<DeployParam>, String> {
    let mut params = Vec::with_capacity(args.len());
    for arg in args {
        let (kind, rest) = arg.split_once(':').ok_or_else(|| {
            format!(
                "the deploy param '{arg}' is not typed, write addr:, u64:, u128:, or guardians:"
            )
        })?;
        let param = match kind {
            "addr" => DeployParam::Address(address_payload(rest)?),
            "u64" => DeployParam::U64(
                rest.parse()
                    .map_err(|_| format!("the u64 param '{rest}' is not a number"))?,
            ),
            "u128" => DeployParam::U128(
                rest.parse()
                    .map_err(|_| format!("the u128 param '{rest}' is not a number"))?,
            ),
            "guardians" => {
                let mut gs = Vec::new();
                for a in rest.split(',').filter(|s| !s.is_empty()) {
                    gs.push(address_payload(a)?);
                }
                if gs.is_empty() {
                    return Err("a guardians param needs at least one Q1 address".to_string());
                }
                DeployParam::Guardians(gs)
            }
            other => {
                return Err(format!(
                    "unknown deploy param type '{other}', use addr, u64, u128, or guardians"
                ))
            }
        };
        params.push(param);
    }
    Ok(params)
}

fn parse_selector(text: &str) -> Result<[u8; 4], String> {
    let bytes = from_hex(text)?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "a selector is four bytes of hex".to_string())
}

// An order field is offset:type:value, laid into the signed order region at the offset the entry reads.
// type is u64, u128, addr for a Q1 address, or name for a bare label.
fn parse_order_fields(specs: &[String]) -> Result<Vec<FieldArg>, String> {
    let mut fields = Vec::with_capacity(specs.len());
    for spec in specs {
        let mut parts = spec.splitn(3, ':');
        let off = parts
            .next()
            .ok_or_else(|| format!("the field '{spec}' has no offset"))?;
        let kind = parts
            .next()
            .ok_or_else(|| format!("the field '{spec}' has no type"))?;
        let val = parts
            .next()
            .ok_or_else(|| format!("the field '{spec}' has no value"))?;
        let offset: u64 = off
            .parse()
            .map_err(|_| format!("the field offset '{off}' is not a number"))?;
        let value = match kind {
            "u64" => FieldValue::Word(
                val.parse()
                    .map_err(|_| format!("the u64 field '{val}' is not a number"))?,
            ),
            "u128" => FieldValue::wide(
                val.parse()
                    .map_err(|_| format!("the u128 field '{val}' is not a number"))?,
            ),
            "addr" => FieldValue::Address(address_payload(val)?),
            "name" => FieldValue::name(val),
            other => {
                return Err(format!(
                    "unknown field type '{other}', use u64, u128, addr, or name"
                ))
            }
        };
        fields.push(FieldArg { offset, value });
    }
    Ok(fields)
}

fn cmd_asset(args: &[String], flags: &Flags) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("") {
        "balance" => {
            if args.len() < 3 {
                return Err("usage: qtv asset balance <issuer> <holder>".to_string());
            }
            let (issuer, holder) = (&args[1], &args[2]);
            let balance = Client::new(flags.gateway.clone()).asset_balance(issuer, holder)?;
            println!("issuer  {issuer}");
            println!("holder  {holder}");
            println!("balance {balance}");
            Ok(())
        }
        _ => Err("usage: qtv asset balance <issuer> <holder>".to_string()),
    }
}

fn cmd_events(args: &[String], flags: &Flags) -> Result<(), String> {
    let height: u64 = args
        .first()
        .ok_or("usage: qtv events <height>")?
        .parse()
        .map_err(|_| "the height is not a number")?;
    let events = Client::new(flags.gateway.clone()).events(height)?;
    println!("events {}", events.len());
    for event in events {
        println!(
            "  {} {} {}",
            event.contract,
            to_hex(&event.selector),
            to_hex(&event.data)
        );
    }
    Ok(())
}

/// A contract call needs a contract sized budget. The flag defaults to the native
/// transfer meter, which is a budget for moving coins and cannot execute a contract,
/// so an unflagged call was charged a fee and its nonce bumped for a call that could
/// never have succeeded.
/// Matches the budget a deploy is given, which is the per transaction VM ceiling.
const CONTRACT_CALL_METER: u64 = 12_000_000;

fn call_meter(flags: &Flags) -> u64 {
    if flags.meter == qcore::NATIVE_TRANSFER_METER {
        CONTRACT_CALL_METER
    } else {
        flags.meter
    }
}

fn deploy_meter(flags: &Flags) -> u64 {
    // deploy carries the whole container, so give it room above a bare transfer unless the caller set one
    if flags.meter == qcore::NATIVE_TRANSFER_METER {
        12_000_000
    } else {
        flags.meter
    }
}

fn report_submit(verb: &str, outcome: Submit) -> Result<(), String> {
    match outcome {
        Submit::Accepted { state, tx_id } => {
            println!("{verb} {tx_id}");
            println!("state    {state}");
            Ok(())
        }
        Submit::Rejected {
            reason,
            expected,
            got,
        } => {
            let mut message = format!("the node rejected the transaction: {reason}");
            if let (Some(expected), Some(got)) = (expected, got) {
                message.push_str(&format!(" (expected nonce {expected}, got {got})"));
            }
            Err(message)
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim().strip_prefix("0x").unwrap_or(text.trim());
    if !text.len().is_multiple_of(2) {
        return Err("the hex has an odd length".to_string());
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks(2) {
        let s = std::str::from_utf8(pair).map_err(|_| "the value is not hex")?;
        out.push(u8::from_str_radix(s, 16).map_err(|_| "the value is not hex")?);
    }
    Ok(out)
}

fn print_usage() {
    println!("qtv, the Quantova command line client");
    println!();
    println!("usage");
    println!("  qtv <command> [flags]");
    println!();
    println!("commands");
    println!("  key new                          create an account, its seed, phrase, and address");
    println!("  key address [<key>]              the address for a key and index");
    println!("  key pubkey  [<key>]              the scheme, public key, and address, for genesis");
    println!("  key restore <phrase>             recover a seed and address from a phrase");
    println!("  account <address>                an account balance, nonce, scheme, and key state");
    println!("  register                         register the account key so it can send");
    println!("  send <to> <amount>               sign and submit a native transfer");
    println!("  info                             the chain id, height, fee, and version");
    println!("  tx <tx-id>                       where a transaction is");
    println!(
        "  contract deploy <file> [param]   deploy a Quanta container with genesis deploy params"
    );
    println!(
        "  contract call <address> <hex>    call a contract, add --value <n> for a paid entry"
    );
    println!("  contract order <address> <sel>   submit an owner or operator signed order");
    println!("  contract storage <address>       read a contract storage slots");
    println!("  asset balance <issuer> <holder>  a holder balance of an issuer's asset");
    println!("  events <height>                  the contract events in a block");
    println!("  version                          the qtv version");
    println!();
    println!("deploy params");
    println!("  addr:<Q1>            a thirty two byte address argument");
    println!("  u64:<n>             an eight byte word argument");
    println!("  u128:<n>            a sixteen byte wide argument");
    println!("  guardians:<Q1,Q1>   a guardian set, comma separated");
    println!("  list them in the order the contract's genesis reads deploy_params");
    println!();
    println!("flags");
    println!(
        "  -g, --gateway <url>   the gateway to talk to, or QTV_GATEWAY, default {DEFAULT_GATEWAY}"
    );
    println!("  -k, --key <value>     a seed hex, a phrase, or @file, or QTV_KEY");
    println!("  -i, --index <n>       the account index under one seed, default 0");
    println!("      --max-fee <n>     the most fee you will pay, required to sign (send, register, contract)");
    println!("      --meter <n>       the execution meter for a contract call");
    println!(
        "      --value <n>       the Quon a paid contract call moves, read by the entry at @value"
    );
    println!("      --asset <issuer>  fund a contract call with an issuer's token instead of the native asset");
    println!("      --scheme-off <n>  the order scheme word offset, for contract order");
    println!("      --ptr-off <n>     the order pointer word offset, for contract order");
    println!(
        "      --field <o:t:v>   an order field, offset:type:value, type u64 u128 addr or name"
    );
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn write_key(mode: u32) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("qtv_key_test_{mode}_{}", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    #[test]
    fn a_group_or_other_readable_key_file_is_refused() {
        let path = write_key(0o644);
        let result = parse_key_value(&format!("@{}", path.display()));
        let _ = std::fs::remove_file(&path);
        assert!(
            result.is_err(),
            "a group or other readable key file must be refused"
        );
        assert!(result.unwrap_err().contains("chmod 600"));
    }

    #[test]
    fn a_private_key_file_at_0600_is_accepted() {
        let path = write_key(0o600);
        let result = parse_key_value(&format!("@{}", path.display()));
        let _ = std::fs::remove_file(&path);
        assert!(
            result.is_ok(),
            "a private key file at 0600 must be accepted"
        );
    }
}
