# Bots, pager and demo

Small Node scripts that sit beside the contract on NEAR **testnet**: a deploy
script, bots that stand in for a community, a pager that announces arrests,
and the runner that plays the whole demo script.

Nothing on-chain depends on any of them. The coin arrests, can be saved and
dies by its own rules whether or not these are running.

```bash
make deploy-demo      # a new demo-preset coin on testnet, left without access keys
make demo             # the demo script, end to end, with a link for every transaction
make pager            # announce arrests, saves and deaths
make holders          # bot-holder-*: a sinus rhythm
make pacer            # bot-pacer: a paced rhythm
make rescue           # bot-compressor and bot-airway: run a code when nobody is watching
```

## The scripts

| Script | Does |
| --- | --- |
| `deploy.mjs <name> [key=value ...]` | Creates a testnet account from the faucet, deploys the wasm and calls `new(config)` in one transaction, creates the bot accounts, registers them and claims the faucet, deletes the coin's only access key and checks that none are left. Writes `deployments/<name>.json`. |
| `holders.mjs [deployment] [seconds]` | `bot-holder-*` send each other 5 to 60 ROSC in turn. Several senders make a sinus rhythm on the monitor. |
| `pacer.mjs [deployment] [hours]` | `bot-pacer` and `bot-pacer-2` pass the smallest transfer that is a beat back and forth, twice per arrest window. The coin stays alive and the monitor labels the rhythm as paced. |
| `rescue.mjs [deployment]` | `bot-compressor` and `bot-airway` wait for an arrest and run the code: one `compress()` per block, both breaths, epinephrine and shocks when offered, a new lead every round, then `claim_bounty()`. `AIRWAY_ONLY=1` leaves the chest to a human. |
| `pager.mjs [deployment]` | Polls `get_monitor()` and posts four things: a bradycardia warning at half the arrest window, `CODE BLUE` with the rhythm and a link to the code screen, ROSC with downtime, rounds and bounty, and each time of death. |
| `demo.mjs` | The demo script. Exits 0 only if every step did what the script says. |

Every bot account is a sub-account of its coin and its name starts with
`bot-`, for example `bot-compressor.rosc-d829d3.testnet`. The deployment file
lists them, so the page can mark their beats: a beat's tag on the trace is the
first four bytes of the SHA-256 of the sender's account id, read as a
little-endian number.

## The demo

`make demo` uses the coins in `deployments/` when they are still usable and
deploys new ones when they are not. A demo-preset coin dies about half an
hour after its last beat, so most runs deploy.

1. The holder bots run for 30 seconds. The monitor shows beats from four senders and the supply falls with each transfer.
2. One more wallet sends a transfer. It appears on the trace at its block.
3. The bots stop. One hundred blocks later the coin is in arrest and a transfer fails with the no-pulse message.
4. The pager has warned at half the window and posted `CODE BLUE`.
5. The rescue bots call the code and run it.
6. A `shock()` on the flatline is refused.
7. ROSC. A transfer works again, both bots claim, and the code sheet is printed.
8. The second coin, launched with a shortened life, has died three times with no call from anyone. One `call_time_of_death()` writes all three deaths.
9. Every later call on it fails with the time of death.

Settings, as environment variables:

| Variable | Effect |
| --- | --- |
| `HUMAN=1` | A person does the CPR on the page. The bots wait for ROSC. Add `AIRWAY=1` to have `bot-airway` give the breaths. |
| `FRESH=1` | Deploy new coins even if usable ones are on file. |
| `HEADSTONE=reuse` | Accept a second coin whose deaths were written on an earlier run. |
| `HOLD_SECONDS` | How long the holder bots run in step 1. Default 30. |
| `NEAR_RPC` | Comma-separated testnet RPC endpoints, in order of preference. |
| `PAGER_WEBHOOK` | A Discord-style webhook for the pager: it receives `{"content": text}`. |
| `PAGER_TELEGRAM_TOKEN`, `PAGER_TELEGRAM_CHAT` | A Telegram bot and chat for the pager, instead of a webhook. |
| `PAGE_URL` | The page the `CODE BLUE` link points at. Default `http://localhost:5173/`. |

It writes two files:

- `out/events.json`: every beat, arrest, counted compression, missed press, breath, refused shock, ROSC and time of death, with the wall-clock time it was sent and the block it landed in.
- `out/gas.json`: gas burnt per call, what a full code cost each rescuer in NEAR, and the landing rate the compressor achieved.

## How the bots send

Each account keeps its own nonce and a recent block hash, signs locally and
broadcasts without waiting (`send_tx` with `wait_until: "NONE"`). The outcome
is read back afterwards with `tx`, through one slow lane shared by all
wallets, so a burst of presses never becomes a burst of status requests. Free
endpoints cut long-held requests short, which is why nothing here waits on the
send itself.

Requests go to the first endpoint in `NEAR_RPC` that answers. A refusal moves
on to the next one and leaves the refused endpoint alone for a while.

## Keys

Keys are written to `bots/.keys/`, which git ignores, with owner-only
permissions. The coin's own key is deleted from the chain and from disk at the
end of `deploy.mjs`. Testnet only: nothing here knows a mainnet endpoint.
