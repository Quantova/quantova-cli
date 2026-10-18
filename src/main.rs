// Copyright 2026 Quantova Inc
// SPDX-License-Identifier: Apache-2.0 OR MIT

use qcore::{
    account_address, account_public_key, address_payload,
    contract::{self, DeployParam, FieldArg, FieldValue, DEFAULT_REGION_OFFSET},
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

struct Flags {
    gateway: String,
    key: Option<Zeroizing<String>>,
    key_on_argv: bool,
    index: u64,
    max_fee: u128,
    max_fee_set: bool,
    meter: Option<u64>,
    value: u64,
    asset: Option<String>,
    scheme_off: Option<u64>,
    ptr_off: Option<u64>,
    expiry_off: Option<u64>,
    expiry: Option<u64>,
    layout_hash: Option<String>,
    order_nonce: Option<u64>,
    fields: Vec<String>,
    network: Option<String>,
    acknowledge_mainnet: bool,
    insecure_argv: bool,
    show_seed: bool,
    known: Option<String>,
    roster: Option<String>,
    genesis: Option<String>,
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
        max_fee: 0,
        max_fee_set: false,
        meter: None,
        value: 0,
        asset: None,
        scheme_off: None,
        ptr_off: None,
        expiry_off: None,
        expiry: None,
        layout_hash: None,
        order_nonce: None,
        fields: Vec::new(),
        network: None,
        acknowledge_mainnet: false,
        insecure_argv: false,
        show_seed: false,
        known: None,
        roster: None,
        genesis: None,
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
            "--index" | "-i" => flags.index = whole(&value("--index")?, "the index")?,
            "--max-fee" => {
                flags.max_fee = whole(&value("--max-fee")?, "the max fee")?;
                flags.max_fee_set = true;
            }
            "--meter" => flags.meter = Some(whole(&value("--meter")?, "the meter")?),
            "--value" => flags.value = whole(&value("--value")?, "the value")?,
            "--asset" => flags.asset = Some(value("--asset")?),
            "--scheme-off" => {
                flags.scheme_off = Some(whole(&value("--scheme-off")?, "the scheme offset")?)
            }
            "--ptr-off" => flags.ptr_off = Some(whole(&value("--ptr-off")?, "the pointer offset")?),
            "--expiry-off" => {
                flags.expiry_off = Some(whole(&value("--expiry-off")?, "the expiry offset")?)
            }
            "--expiry" => flags.expiry = Some(whole(&value("--expiry")?, "the expiry time")?),
            "--layout-hash" => flags.layout_hash = Some(value("--layout-hash")?),
            "--order-nonce" => {
                flags.order_nonce = Some(whole(&value("--order-nonce")?, "the order nonce")?)
            }
            "--field" => flags.fields.push(value("--field")?),
            "--network" => flags.network = Some(value("--network")?),
            "--acknowledge-mainnet" => flags.acknowledge_mainnet = true,
            "--insecure-argv" => flags.insecure_argv = true,
            "--show-seed" => flags.show_seed = true,
            "--known" => flags.known = Some(value("--known")?),
            "--roster" => flags.roster = Some(value("--roster")?),
            "--genesis" => flags.genesis = Some(value("--genesis")?),
            _ => rest.push(arg.clone()),
        }
        i += 1;
    }
    Ok((flags, rest))
}

fn whole<T: TryFrom<u128>>(text: &str, what: &str) -> Result<T, String> {
    qcore::whole_number(text)
        .and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| format!("{what} is not a whole number in range"))
}

fn address_book(flags: &Flags) -> Result<Vec<String>, String> {
    let Some(path) = &flags.known else {
        return Ok(Vec::new());
    };
    let book =
        std::fs::read_to_string(path).map_err(|e| format!("read the address book {path}: {e}"))?;
    Ok(book
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect())
}

fn genesis_flag(flags: &Flags) -> Result<Option<[u8; 32]>, String> {
    match flags.genesis.as_deref() {
        None => Ok(None),
        Some(hex) => qcore::json::from_hex(hex)?
            .try_into()
            .map(Some)
            .map_err(|_| "the genesis hash must be 32 bytes of hex".to_string()),
    }
}

fn open_client(flags: &Flags) -> Result<Client, String> {
    let client = open_network_client(flags)?.with_address_book(&address_book(flags)?)?;
    if let Some(path) = &flags.roster {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read the validator roster {path}: {e}"))?;
        let genesis = genesis_flag(flags)?.ok_or(
            "a trusted roster needs the genesis hash it belongs to, pass --genesis <64 hex characters>",
        )?;
        client.trust_roster(genesis, qcore::finality::parse_roster(&text)?)?;
    }
    Ok(client)
}

fn pinned(mut network: qcore::Network, flags: &Flags) -> Result<qcore::Network, String> {
    if let Some(genesis) = genesis_flag(flags)? {
        match network.genesis_hash {
            Some(known) if known != genesis => {
                return Err(
                    "the --genesis hash differs from the genesis this release pins for the network"
                        .to_string(),
                )
            }
            _ => network.genesis_hash = Some(genesis),
        }
    }
    Ok(network)
}

fn open_network_client(flags: &Flags) -> Result<Client, String> {
    let gateway = flags.gateway.clone();
    match flags.network.as_deref() {
        None => Ok(Client::new(gateway)),
        Some("testnet") => Ok(Client::with_network(
            gateway,
            pinned(qcore::Network::testnet(), flags)?,
            false,
        )),
        Some("mainnet") => {
            if !flags.acknowledge_mainnet {
                return Err("signing for mainnet needs an explicit --acknowledge-mainnet alongside --network mainnet".to_string());
            }
            Ok(Client::with_network(
                gateway,
                pinned(qcore::Network::mainnet(), flags)?,
                true,
            ))
        }
        Some(other) => Err(format!(
            "unknown network '{other}', use --network testnet or --network mainnet"
        )),
    }
}

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

fn read_phrase(args: &[String], insecure: bool) -> Result<Zeroizing<String>, String> {
    use std::io::Read;
    if args.is_empty() {
        let mut body = Zeroizing::new(String::new());
        std::io::stdin()
            .read_to_string(&mut body)
            .map_err(|e| format!("read the phrase from stdin: {e}"))?;
        return Ok(Zeroizing::new(body.trim().to_string()));
    }
    if args.len() == 1 {
        if let Some(path) = args[0].trim().strip_prefix('@') {
            let mut file =
                std::fs::File::open(path).map_err(|e| format!("read the phrase file: {e}"))?;
            refuse_if_key_file_is_shared(&file, path)?;
            let mut body = Zeroizing::new(String::new());
            file.read_to_string(&mut body)
                .map_err(|e| format!("read the phrase file: {e}"))?;
            return Ok(Zeroizing::new(body.trim().to_string()));
        }
    }
    let phrase = Zeroizing::new(args.join(" "));
    refuse_key_on_argv(&phrase, insecure)?;
    Ok(phrase)
}

fn refuse_key_on_argv(raw: &str, insecure: bool) -> Result<(), String> {
    if raw.trim_start().starts_with('@') {
        return Ok(());
    }
    if insecure {
        eprintln!(
            "warning: a key on the command line is visible to other users through the process list \
             and shell history; prefer @file, stdin, or the QTV_KEY environment variable"
        );
        return Ok(());
    }
    Err(
        "a key on the command line is visible to other users through the process list and shell \
         history; pass it as @file, from stdin, or in the QTV_KEY environment variable, or repeat \
         the command with --insecure-argv to override"
            .to_string(),
    )
}

fn resolve_key(flags: &Flags) -> Result<Zeroizing<[u8; 32]>, String> {
    let raw = flags
        .key
        .clone()
        .ok_or("no key given, pass --key <seed-or-phrase> or set QTV_KEY")?;
    if flags.key_on_argv {
        refuse_key_on_argv(&raw, flags.insecure_argv)?;
    }
    parse_key_value(&raw)
}

fn refuse_if_key_file_is_shared(file: &std::fs::File, path: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = file
            .metadata()
            .map_err(|e| format!("read the key file: {e}"))?;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "the key file {path} is readable by group or others, restrict it with chmod 600 before use"
            ));
        }
    }
    let _ = (file, path);
    Ok(())
}

fn parse_key_value(raw: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    use std::io::Read;
    let raw = raw.trim();
    if let Some(path) = raw.strip_prefix('@') {
        let mut file = std::fs::File::open(path).map_err(|e| format!("read the key file: {e}"))?;
        refuse_if_key_file_is_shared(&file, path)?;
        let mut body = Zeroizing::new(String::new());
        file.read_to_string(&mut body)
            .map_err(|e| format!("read the key file: {e}"))?;
        return parse_key_text(&body);
    }
    parse_key_text(raw)
}

fn parse_key_text(raw: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let raw = raw.trim();
    if raw.split_whitespace().count() >= 2 {
        return seed_from_mnemonic(raw);
    }
    parse_seed_hex(raw)
}

fn parse_seed_hex(hex: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(
            "a seed is sixty four hex characters, or pass a twenty five word phrase".to_string(),
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
            let phrase = mnemonic_from_seed(&seed);
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
            let phrase = read_phrase(&args[1..], flags.insecure_argv)?;
            if phrase.trim().is_empty() {
                return Err("usage: qtv key restore [<twenty five word phrase> | @file]; omit the phrase to read it from stdin".to_string());
            }
            let seed = seed_from_mnemonic(&phrase)?;
            if flags.show_seed {
                let seed_hex = Zeroizing::new(to_hex(&seed[..]));
                println!("seed    {}", seed_hex.as_str());
            }
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
            refuse_key_on_argv(value, flags.insecure_argv)?;
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
    let account = open_client(flags)?.account(address)?;
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
    let (_signed, outcome) = open_client(flags)?.register(&seed, flags.index, max_fee)?;
    report_submit("registered", outcome)
}

fn cmd_send(args: &[String], flags: &Flags) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: qtv send <to> <amount> --key <seed-or-phrase>".to_string());
    }
    let to = &args[0];
    let amount: u64 = whole(&args[1], "the amount")?;
    let seed = resolve_key(flags)?;
    let max_fee = require_max_fee(flags)?;
    let (_signed, outcome) =
        open_client(flags)?.transfer(&seed, flags.index, to, amount, max_fee)?;
    report_submit("submitted", outcome)
}

fn cmd_info(flags: &Flags) -> Result<(), String> {
    let info = open_client(flags)?.node_info()?;
    println!("chain   {}", info.chain_id);
    println!("genesis {}", info.genesis_hash);
    println!("height  {}", info.head_height);
    println!("fee     {} {}", info.transfer_fee, info.denomination);
    println!("version {}", info.version);
    Ok(())
}

fn cmd_tx(args: &[String], flags: &Flags) -> Result<(), String> {
    let tx_id = args.first().ok_or("usage: qtv tx <tx-id>")?;
    match open_client(flags)?.transaction(tx_id)? {
        TxStatus::Finalised {
            height,
            block: Some(block),
        } => println!("finalised at height {height} in block {block}"),
        TxStatus::Finalised {
            height,
            block: None,
        } => println!("finalised at height {height}, its block is no longer served"),
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
            let meter = required_meter(flags, "deploy")?;
            let (_signed, outcome, address) = open_client(flags)?.deploy_with_params(
                &seed,
                flags.index,
                &container,
                &params,
                meter,
                max_fee,
            )?;
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
            let meter = required_meter(flags, "call")?;
            let client = open_client(flags)?;
            let (_signed, outcome) = match &flags.asset {
                Some(issuer) => client.call_asset(
                    &seed,
                    flags.index,
                    target,
                    call_args,
                    issuer,
                    flags.value,
                    meter,
                    max_fee,
                )?,
                None => client.call_payable(
                    &seed,
                    flags.index,
                    target,
                    call_args,
                    flags.value,
                    meter,
                    max_fee,
                )?,
            };
            report_submit("called", outcome)
        }
        "order" => {
            if args.len() < 3 {
                return Err(
                    "usage: qtv contract order <address> <selector-hex> --scheme-off <n> \
                            --ptr-off <n> --expiry-off <n> --layout-hash <hex> [--order-nonce <n>] [--expiry <unix seconds>] \
                            --field <offset:type:value> ... --key <owner>"
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
            let expiry_off = flags
                .expiry_off
                .ok_or("pass --expiry-off <n>, the order expiry word offset")?;
            let layout_hash =
                contract::layout_hash_from_hex(flags.layout_hash.as_deref().ok_or(
                    "pass --layout-hash <hex>, the order layout hash the contract checks",
                )?)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "the system clock is before 1970".to_string())?
                .as_secs();
            let expiry = flags.expiry.unwrap_or_else(|| now.saturating_add(3600));
            contract::check_order_expiry(expiry, now)?;
            let fields = parse_order_fields(&flags.fields)?;
            let seed = resolve_key(flags)?;
            let max_fee = require_max_fee(flags)?;
            let meter = required_meter(flags, "order")?;
            let (_signed, outcome, _order) = open_client(flags)?.call_typed_order_expecting(
                &seed,
                flags.index,
                target,
                selector,
                &layout_hash,
                scheme_off,
                ptr_off,
                expiry_off,
                expiry,
                DEFAULT_REGION_OFFSET,
                &fields,
                &seed,
                flags.index,
                flags.value,
                flags.asset.as_deref(),
                meter,
                max_fee,
                None,
                flags.order_nonce,
            )?;
            report_submit("ordered", outcome)
        }
        "storage" => {
            let address = args.get(1).ok_or("usage: qtv contract storage <address>")?;
            let slots = open_client(flags)?.storage(address)?;
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
            "u64" => DeployParam::U64(whole(rest, &format!("the u64 param '{rest}'"))?),
            "u128" => DeployParam::U128(whole(rest, &format!("the u128 param '{rest}'"))?),
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
        let offset: u64 = whole(off, &format!("the field offset '{off}'"))?;
        let value = match kind {
            "u64" => FieldValue::Word(whole(val, &format!("the u64 field '{val}'"))?),
            "u128" => FieldValue::wide(whole(val, &format!("the u128 field '{val}'"))?),
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
            let balance = open_client(flags)?.asset_balance(issuer, holder)?;
            println!("issuer  {issuer}");
            println!("holder  {holder}");
            println!("balance {balance}");
            Ok(())
        }
        _ => Err("usage: qtv asset balance <issuer> <holder>".to_string()),
    }
}

fn cmd_events(args: &[String], flags: &Flags) -> Result<(), String> {
    let height: u64 = whole(
        args.first().ok_or("usage: qtv events <height>")?,
        "the height",
    )?;
    let events = open_client(flags)?.events(height)?;
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

fn fee_units(meter: u64) -> u64 {
    meter.div_ceil(qcore::NATIVE_TRANSFER_METER)
}

fn required_meter(flags: &Flags, what: &str) -> Result<u64, String> {
    let meter = flags.meter.ok_or_else(|| {
        format!(
            "pass --meter <n>, the execution meter for this {what}; the fee you authorise is one transfer fee for every {} meter, so size it to the {what} rather than signing a large default",
            qcore::NATIVE_TRANSFER_METER
        )
    })?;
    eprintln!(
        "meter {meter} authorises a fee of up to {} transfer fees",
        fee_units(meter)
    );
    Ok(meter)
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
    if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the value is not hex".to_string());
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
    println!("      --insecure-argv   allow a raw key on the command line (refused by default)");
    println!("  -i, --index <n>       the account index under one seed, default 0");
    println!("      --max-fee <n>     the most fee you will pay, required to sign (send, register, contract)");
    println!(
        "      --network <name>  testnet or mainnet, required to sign for either public network"
    );
    println!(
        "      --meter <n>       the execution meter, required for contract deploy, call and order"
    );
    println!("      --known <file>    an address book, one Q1 address per line, a lookalike recipient is refused");
    println!("      --roster <file>   a validator roster you trust, the head is then checked against its finality proof before signing, required on mainnet");
    println!("      --genesis <hex>   the network genesis hash to sign under and the one the trusted roster belongs to");
    println!(
        "      --value <n>       the Quon a paid contract call moves, read by the entry at @value"
    );
    println!("      --asset <issuer>  fund a contract call with an issuer's token instead of the native asset");
    println!("      --scheme-off <n>  the order scheme word offset, for contract order");
    println!("      --ptr-off <n>     the order pointer word offset, for contract order");
    println!("      --expiry-off <n>  the order expiry word offset, for contract order");
    println!("      --expiry <secs>   the unix time the order stops being valid, within 24 hours, default one hour from now");
    println!(
        "      --layout-hash <hex>  the order layout hash the contract checks, for contract order"
    );
    println!(
        "      --order-nonce <n>    the order nonce the contract expects next, needed once it is above zero"
    );
    println!(
        "      --field <o:t:v>   an order field, offset:type:value, type u64 u128 addr or name"
    );
}

#[cfg(test)]
mod hex_tests {
    use super::*;

    #[test]
    fn hex_with_a_sign_character_is_refused() {
        assert!(from_hex("+a+b").is_err());
        assert!(parse_seed_hex(&format!("+a{}", "b".repeat(62))).is_err());
        assert_eq!(from_hex("0x0aff").unwrap(), vec![0x0a, 0xff]);
        assert!(whole::<u64>("+5", "x").is_err());
        assert!(whole::<u64>(" 5", "x").is_err());
        assert_eq!(whole::<u64>("5", "x"), Ok(5));
    }

    #[test]
    fn a_contract_call_without_a_meter_is_refused_rather_than_defaulted() {
        let (flags, _) = parse_flags(&[]).unwrap();
        let refused = required_meter(&flags, "call").unwrap_err();
        assert!(refused.contains("--meter"), "{refused}");
        let (flags, _) = parse_flags(&["--meter".to_string(), "60500".to_string()]).unwrap();
        assert_eq!(required_meter(&flags, "call").unwrap(), 60_500);
    }
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
    fn a_key_file_naming_another_file_is_refused_not_followed() {
        let path = std::env::temp_dir().join(format!("qtv_key_loop_{}", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(format!("@{}", path.display()).as_bytes())
            .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let result = parse_key_value(&format!("@{}", path.display()));
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
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
