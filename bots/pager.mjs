// The pager: watches `get_monitor()` and tells people when the coin needs them.
//
//   node bots/pager.mjs [deployment]         default: testnet-demo
//
// Where it posts:
//   PAGER_WEBHOOK=<url>                       a Discord-style webhook: POST {"content": text}
//   PAGER_TELEGRAM_TOKEN + PAGER_TELEGRAM_CHAT  a Telegram bot's sendMessage
//   neither                                   standard output
//   PAGE_URL=<url>                            the page the CODE BLUE link points at
//
// A convenience. Nothing on-chain depends on it: the coin arrests, and can be
// saved, whether or not anyone is paged.
import { fileURLToPath } from "node:url";
import { Poller } from "./lib/coin.mjs";
import { block, loadDeployment, measureBlockMs, rosc, statusOf, view } from "./lib/near.mjs";

export function poster() {
  const { PAGER_WEBHOOK, PAGER_TELEGRAM_TOKEN, PAGER_TELEGRAM_CHAT } = process.env;
  const send = async (url, body) => {
    const res = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
    if (!res.ok) console.error(`[pager] the webhook answered ${res.status}`);
  };
  if (PAGER_TELEGRAM_TOKEN && PAGER_TELEGRAM_CHAT) {
    return (text) => send(`https://api.telegram.org/bot${PAGER_TELEGRAM_TOKEN}/sendMessage`, { chat_id: PAGER_TELEGRAM_CHAT, text });
  }
  if (PAGER_WEBHOOK) return (text) => send(PAGER_WEBHOOK, { content: text });
  return async (text) => console.log(`[pager] ${text}`);
}

const span = (blocks, blockMs) => {
  const s = Math.round((blocks * blockMs) / 1000);
  return s < 120 ? `${s} s` : s < 7200 ? `${Math.round(s / 60)} min` : `${(s / 3600).toFixed(1)} h`;
};

export class Pager {
  constructor({ contract, post = poster(), pageUrl = process.env.PAGE_URL || "http://localhost:5173/", blockMs = 600 }) {
    Object.assign(this, { contract, post, pageUrl, blockMs });
    this.posts = [];
    this.warnedFor = null; // last_beat_block of the silence already warned about
    this.code = null; // the code last seen running
    this.deaths = null;
    this.busy = Promise.resolve();
  }

  say(kind, text) {
    this.posts.push({ t: new Date().toISOString(), kind, text });
    return this.post(text);
  }

  /** Feed it every monitor. Posts happen in order, one at a time. */
  observe(m) {
    this.busy = this.busy.then(() => this.step(m)).catch((e) => console.error(`[pager] ${e.message}`));
    return this.busy;
  }

  async step(m) {
    const status = statusOf(m);
    const name = this.contract;
    if (this.deaths === null) this.deaths = m.deaths;

    if (status === "Alive") {
      const silent = m.block - m.last_beat_block;
      const left = m.status.Alive.arrest_block - m.block;
      if (silent >= m.config.arrest_blocks / 2 && this.warnedFor !== m.last_beat_block) {
        this.warnedFor = m.last_beat_block;
        await this.say("bradycardia", `BRADYCARDIA on ${name}. No beat for ${silent} blocks. Arrest in ${left} blocks, about ${span(left, this.blockMs)}, unless someone transfers.`);
      }
      if (this.code) {
        // The code that was running is over and there is a pulse.
        const code = this.code;
        this.code = null;
        const sheets = await view(name, "get_code_sheets").catch(() => []);
        const sheet = sheets.find((s) => s.code_no === code.code_no);
        const downtime = m.alive_since_block - code.arrest_block;
        const pay = sheet
          ? `Bounty ${rosc(sheet.bounty)} ROSC to ${sheet.shares.length} rescuer${sheet.shares.length === 1 ? "" : "s"}.`
          : "The bounty is credited with the next call.";
        await this.say("rosc", `ROSC on ${name}. Pulse back after ${downtime} blocks, about ${span(downtime, this.blockMs)}, in ${sheet ? sheet.rounds.length : code.round} round${(sheet ? sheet.rounds.length : code.round) === 1 ? "" : "s"}. ${pay}`);
      }
    }

    if (m.deaths > this.deaths) {
      const at = status === "Deceased" ? m.status.Deceased.block : status === "Morgue" ? m.status.Morgue.since_block : null;
      let date = "";
      if (at) {
        const b = await block({ block_id: at }).catch(() => null);
        if (b) date = `, ${new Date(Number(BigInt(b.header.timestamp_nanosec) / 1_000_000n)).toISOString().replace("T", " ").slice(0, 19)} UTC`;
      }
      const final = status === "Deceased" ? " The coin is deceased. Transfers are frozen for good." : "";
      await this.say("time_of_death", `TIME OF DEATH on ${name}${at ? `: block ${at}${date}` : ""}. Death ${m.deaths} of ${m.max_deaths}.${final}`);
      this.deaths = m.deaths;
      this.code = null;
    }

    if (status === "Arrest" && this.code?.code_no !== m.code.code_no) {
      this.code = { code_no: m.code.code_no, arrest_block: m.code.arrest_block, round: m.code.round };
      const link = `${this.pageUrl}${this.pageUrl.includes("?") ? "&" : "?"}contract=${name}#code`;
      await this.say("code_blue", `CODE BLUE on ${name}. No pulse since block ${m.code.arrest_block}. Rhythm: ${m.code.rhythm.toLowerCase()}. Start compressions: ${link}`);
    } else if (status === "Arrest") {
      this.code.round = m.code.round;
    }
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const deployment = loadDeployment(process.argv[2] || "testnet-demo");
  const pager = new Pager({ contract: deployment.contract, blockMs: await measureBlockMs() });
  const poller = await new Poller(deployment.contract, 2_000).start();
  console.log(`paging for ${deployment.contract}`);
  poller.on((m) => pager.observe(m));
}
