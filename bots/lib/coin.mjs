// Watching one coin: a poller for `get_monitor()` and a log of what happened.
import fs from "node:fs";
import path from "node:path";
import { OUT, blockInfo, rosc, sleep, statusOf, view, writeJson } from "./near.mjs";

/** Keeps the newest `get_monitor()` answer at hand. One request per interval. */
export class Poller {
  constructor(contractId, intervalMs = 600) {
    this.contractId = contractId;
    this.intervalMs = intervalMs;
    this.latest = null;
    this.listeners = new Set();
    this.waiters = [];
    this.running = false;
    this.failures = 0;
  }

  async start() {
    this.latest = await view(this.contractId, "get_monitor");
    this.running = true;
    this.loop = (async () => {
      while (this.running) {
        await sleep(this.intervalMs);
        try {
          const m = await view(this.contractId, "get_monitor");
          // Endpoints can answer out of order. Never step back.
          if (m.block >= this.latest.block) {
            this.latest = m;
            for (const listener of this.listeners) listener(m);
            for (const resolve of this.waiters.splice(0)) resolve(m);
          }
        } catch {
          this.failures++;
        }
      }
    })();
    return this;
  }

  stop() {
    this.running = false;
  }

  on(listener) {
    this.listeners.add(listener);
    listener(this.latest);
    return this;
  }

  /** The next fresh answer. */
  next() {
    return new Promise((resolve) => this.waiters.push(resolve));
  }

  /** Resolves with the first monitor for which `test` holds. */
  async until(test, { timeoutMs = 300_000, what = "the condition" } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (let m = this.latest; ; m = await this.next()) {
      if (test(m)) return m;
      if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}; the coin is ${describe(m)}`);
    }
  }
}

export function describe(m) {
  const status = statusOf(m);
  if (status === "Alive") return `alive, ${m.status.Alive.arrest_block - m.block} blocks from arrest`;
  if (status === "Arrest") return `in arrest (${m.code.rhythm}, round ${m.code.round}, perfusion ${m.code.perfusion / 100}/${m.code.target / 100})`;
  if (status === "Morgue") return `in the morgue until block ${m.status.Morgue.until_block}`;
  return `deceased since block ${m.status.Deceased.block}`;
}

/** A wall-clock log of chain events, for the terminal now and the video edit later. */
export class EventLog {
  constructor(file = path.join(OUT, "events.json")) {
    this.file = file;
    this.events = [];
    this.pending = [];
  }

  /** `at` is a block hash or height to resolve in the background, if there is one. */
  add(kind, fields = {}, { blockHash = null, t = Date.now() } = {}) {
    const event = { t: new Date(t).toISOString(), kind, ...fields };
    this.events.push(event);
    if (blockHash) {
      this.pending.push(
        blockInfo(blockHash)
          .then((info) => Object.assign(event, { block: info.height, block_time: info.time }))
          .catch(() => {}),
      );
    }
    return event;
  }

  async save(extra = {}) {
    await Promise.all(this.pending);
    const events = [...this.events].sort((a, b) => a.t.localeCompare(b.t));
    writeJson(this.file, { ...extra, events });
    return events;
  }
}

export function loadJson(file, fallback = null) {
  return fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, "utf8")) : fallback;
}

export const points = (hundredths) => (hundredths / 100).toFixed(hundredths % 100 ? 2 : 0);

/** One code sheet as lines of text. */
export function sheetLines(sheet, short = (id) => id.split(".")[0]) {
  const lines = [];
  const noFlow = sheet.first_compression_block === null ? null : sheet.first_compression_block - sheet.arrest_block;
  lines.push(`code ${sheet.code_no}: ${sheet.rhythm_at_arrest.toLowerCase()} at block ${sheet.arrest_block}, ended at block ${sheet.end_block}`);
  lines.push(`  downtime ${sheet.end_block - sheet.arrest_block} blocks${noFlow === null ? ", no compression ever landed" : `, ${noFlow} of them before the first compression`}`);
  lines.push(`  outcome ${typeof sheet.outcome === "string" ? sheet.outcome : `death (${Object.values(sheet.outcome)[0]})`}: perfusion ${points(sheet.perfusion)} against a target of ${points(sheet.target)}`);
  for (const r of sheet.rounds) {
    const extras = [r.fatigued ? "same lead as the round before" : null, r.shock === null ? null : r.shock ? "shock" : "shock, no effect", r.epi ? "epinephrine" : null].filter(Boolean);
    lines.push(`  round ${r.round}: quality ${(r.quality_bps / 100).toFixed(1)}%, lead ${r.lead ? short(r.lead) : "nobody"}${extras.length ? `, ${extras.join(", ")}` : ""}, perfusion ${points(r.perfusion_after)}`);
  }
  if (sheet.units.length) lines.push(`  work units: ${sheet.units.map(([who, n]) => `${short(who)} ${n}`).join(", ")}`);
  if (BigInt(sheet.bounty) > 0n) {
    lines.push(`  bounty ${rosc(sheet.bounty)} ROSC: ${sheet.shares.map(([who, share]) => `${short(who)} ${rosc(share)}`).join(", ")}`);
  }
  const burns = [["no-flow", sheet.noflow_burn], ["epinephrine", sheet.epi_burn], ["time of death", sheet.death_burn]].filter(([, v]) => BigInt(v) > 0n);
  if (burns.length) lines.push(`  burned from the cart: ${burns.map(([k, v]) => `${k} ${rosc(v)}`).join(", ")}`);
  return lines;
}
