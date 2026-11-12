// Launches a coin on NEAR testnet and leaves it without an owner.
//
//   node bots/deploy.mjs <name> [--preset demo|live] [--bots full|minimal] [key=value ...]
//
//   node bots/deploy.mjs testnet-demo
//   node bots/deploy.mjs testnet-headstone --bots minimal no_flow_max_blocks=60 lazarus_blocks=20
//
// What it does, in order:
//   1. creates a fresh testnet account for the coin, funded by the faucet;
//   2. deploys the wasm and calls `new(config)` in the same transaction;
//   3. creates the bot accounts as sub-accounts, registers them and claims the faucet;
//   4. deletes the coin account's only access key and checks that none are left;
//   5. writes deployments/<name>.json.
//
// Testnet only. Keys stay in bots/.keys/, which git ignores.
import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KeyPair, actions } from "near-api-js";
import {
  DEPLOYMENTS, ENDPOINTS, NEAR, ROOT, TGAS, Wallet, accessKeys, blockInfo, explorerAccount, explorerTx, hasKey,
  removeKey, rosc, saveKey, sleep, view, viewAccount, writeJson,
} from "./lib/near.mjs";

const WASM = path.join(ROOT, "contracts/rosc/target/near/rosc.wasm");

// Role, and the NEAR each bot starts with. A full code costs the compressor
// about 0.03 NEAR in gas; the rest is headroom for many runs.
const TEAMS = {
  full: [
    ["bot-holder-1", 0.4], ["bot-holder-2", 0.4], ["bot-holder-3", 0.4], ["bot-holder-4", 0.4],
    ["bot-pacer", 0.3], ["bot-pacer-2", 0.3], ["bot-compressor", 1.2], ["bot-airway", 0.8],
  ],
  minimal: [["bot-holder-1", 0.4], ["bot-holder-2", 0.4]],
  // Two wallets and nothing else: a coin kept going by a pacemaker.
  paced: [["bot-pacer", 0.3], ["bot-pacer-2", 0.3]],
};

const log = (...a) => console.log(...a);

function configFor(preset, overrides) {
  const pairs = Object.entries(overrides).map(([k, v]) => `${k}=${v}`);
  const out = execFileSync("cargo", ["run", "-q", "-p", "codeblue-core", "--features", "serde", "--example", "config", "--", preset, ...pairs], {
    cwd: ROOT,
    encoding: "utf8",
  });
  return JSON.parse(out);
}

function near(args) {
  try {
    return execFileSync("near", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], timeout: 120_000 });
  } catch (e) {
    return `${e.stdout || ""}${e.stderr || ""}`;
  }
}

async function createCoinAccount() {
  for (let attempt = 0; attempt < 4; attempt++) {
    const id = `rosc-${crypto.randomBytes(4).toString("hex").slice(0, 6)}.testnet`;
    const keyPair = KeyPair.fromRandom("ed25519");
    saveKey(id, keyPair);
    log(`  asking the testnet faucet for ${id}`);
    const out = near([
      "account", "create-account", "sponsor-by-faucet-service", id,
      "use-manually-provided-public-key", keyPair.getPublicKey().toString(),
      "network-config", "testnet", "create",
    ]);
    for (let i = 0; i < 20; i++) {
      const account = await viewAccount(id).catch(() => null);
      if (account) return { id, balance: BigInt(account.amount) };
      await sleep(1_500);
    }
    removeKey(id);
    log(`  the faucet did not create it: ${out.trim().split("\n").slice(-3).join(" | ")}`);
    await sleep(20_000 * (attempt + 1));
  }
  throw new Error("the testnet faucet refused four times; wait a while, or create an account by hand and pass COIN_ACCOUNT=<id> with its key in bots/.keys/");
}

export async function deploy(name, { preset = "demo", overrides = {}, team = "full", note = null } = {}) {
  if (!fs.existsSync(WASM)) throw new Error(`missing ${WASM}: run \`make build\``);
  const wasm = fs.readFileSync(WASM);
  const config = configFor(preset, overrides);
  log(`deploying ${name}: ${preset} preset${Object.keys(overrides).length ? ` with ${JSON.stringify(overrides)}` : ""}`);

  // 1. The coin's account.
  const { id: coin, balance } = process.env.COIN_ACCOUNT
    ? { id: process.env.COIN_ACCOUNT, balance: BigInt((await viewAccount(process.env.COIN_ACCOUNT)).amount) }
    : await createCoinAccount();
  log(`  ${coin} holds ${Number(balance / 10n ** 21n) / 1000} NEAR`);
  const owner = new Wallet(coin);

  // 2. Code and parameters, in one transaction. (A rerun after a broken
  //    attempt picks the coin up where it was left: DEPLOY_TX names its launch.)
  let launch;
  if (process.env.DEPLOY_TX) {
    launch = await owner.status(process.env.DEPLOY_TX);
    if (!launch?.ok) throw new Error(`DEPLOY_TX ${process.env.DEPLOY_TX} is not a successful launch`);
  } else {
    launch = await owner.send(coin, [actions.deployContract(wasm), actions.functionCall("new", { config }, 100n * TGAS, 0n)]);
    if (!launch.ok) throw new Error(`deploy failed: ${launch.error}`);
  }
  const deployedAt = await blockInfo(launch.blockHash);
  log(`  deployed and initialised at block ${deployedAt.height}: ${explorerTx(launch.hash)}`);

  // 3. The stand-in community.
  const fee = BigInt((await view(coin, "storage_balance_bounds")).min);
  const bots = [];
  for (const [role, funding] of TEAMS[team]) {
    const id = `${role}.${coin}`;
    if (hasKey(id) && (await viewAccount(id).catch(() => null))) {
      bots.push({ id, role, wallet: new Wallet(id), created_tx: null });
      continue;
    }
    const wallet = Wallet.fresh(id);
    const created = await owner.send(id, [
      actions.createAccount(),
      actions.transfer((BigInt(Math.round(funding * 1000)) * NEAR) / 1000n),
      actions.addFullAccessKey(wallet.publicKey),
    ]);
    if (!created.ok) throw new Error(`could not create ${id}: ${created.error}`);
    bots.push({ id, role, wallet, created_tx: created.hash });
  }
  log(`  created ${bots.length} bot accounts`);
  // Each bot registers and claims for itself; three at a time, to stay polite.
  const queue = [...bots];
  await Promise.all(
    Array.from({ length: 3 }, async () => {
      for (let bot = queue.shift(); bot; bot = queue.shift()) {
        const registered = await bot.wallet.call(coin, "storage_deposit", {}, { deposit: fee });
        bot.storage_gas = registered.gas;
        // A coin that has already died, or a bot that already claimed, pays nothing.
        const claim = await bot.wallet.attempt(coin, "faucet_claim");
        bot.faucet_tx = claim.hash;
        bot.claimed = claim.ok ? claim.value : null;
        bot.faucet_gas = claim.ok ? claim.gas : null;
        if (!claim.ok) log(`  ${bot.role} did not get a faucet claim: ${claim.error}`);
      }
    }),
  );
  log(`  ${bots.length} bots registered; ${bots.filter((b) => b.claimed).length} claimed ${rosc(config.faucet_amount)} ROSC each`);

  // A demo-preset coin arrests after a minute of silence, and the launch
  // itself takes most of one. A first beat keeps the pulse going.
  const heartbeat = async () => {
    const [from, to] = bots;
    await from.wallet.attempt(coin, "ft_transfer", { receiver_id: to.id, amount: config.min_stroke }, { deposit: 1n });
  };
  if (team === "full") await heartbeat();

  // 4. No admin: the coin deletes its own last key.
  const before = await accessKeys(coin);
  const cut = await owner.send(coin, [actions.deleteKey(owner.publicKey)]);
  if (!cut.ok) throw new Error(`could not delete the key: ${cut.error}`);
  let keys = before;
  for (let i = 0; i < 20 && keys.length; i++) {
    await sleep(1_500);
    keys = await accessKeys(coin);
  }
  if (keys.length) throw new Error(`${coin} still has ${keys.length} access key(s)`);
  removeKey(coin);
  const command = `near account list-keys ${coin} network-config testnet now`;
  const output = near(["account", "list-keys", coin, "network-config", "testnet", "now"]).trim();
  log(`  ${coin} has no access keys: ${explorerTx(cut.hash)}`);

  // 5. The record.
  if (team === "full") await heartbeat();
  const record = {
    name,
    network: "testnet",
    contract: coin,
    rpc: ENDPOINTS,
    preset,
    overrides,
    note,
    config,
    deployed_at_block: deployedAt.height,
    deployed_at: deployedAt.time,
    deploy_tx: launch.hash,
    wasm_bytes: wasm.length,
    wasm_sha256: crypto.createHash("sha256").update(wasm).digest("hex"),
    bots: bots.map(({ id, role, created_tx, faucet_tx, faucet_gas, storage_gas }) => ({ id, role, created_tx, faucet_tx, faucet_gas, storage_gas })),
    no_admin: {
      keys_before: before.length,
      delete_key_tx: cut.hash,
      keys_after: keys,
      command,
      output,
    },
    supply_at_launch: config.total_supply,
    explorer: {
      contract: explorerAccount(coin),
      deploy_tx: explorerTx(launch.hash),
      delete_key_tx: explorerTx(cut.hash),
    },
  };
  writeJson(path.join(DEPLOYMENTS, `${name}.json`), record);
  log(`  wrote deployments/${name}.json`);
  return record;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const flag = (name, fallback) => {
    const i = args.indexOf(`--${name}`);
    return i >= 0 ? args.splice(i, 2)[1] : fallback;
  };
  const preset = flag("preset", "demo");
  const team = flag("bots", "full");
  const name = args.shift();
  if (!name || !TEAMS[team]) {
    console.error("usage: node bots/deploy.mjs <name> [--preset demo|live] [--bots full|minimal|paced] [key=value ...]");
    process.exit(2);
  }
  const overrides = Object.fromEntries(args.map((pair) => pair.split("=")));
  const record = await deploy(name, { preset, overrides, team });
  console.log(`\n${record.contract}\n${record.explorer.contract}`);
  console.log(`$ ${record.no_admin.command}\n${record.no_admin.output}`);
}
