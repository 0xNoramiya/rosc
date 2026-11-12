# Parameters

Every tunable number in Code Blue lives in `Config`
(`crates/codeblue-core/src/config.rs`). It is written once, in `new`, and no
method writes it afterwards. The contract returns all of it from `get_config()`.

There are two presets. **Demo** runs a whole life in minutes, for the demo
script and the video. **Live** is the preset for a coin left to run. Block
counts assume 600 ms blocks: 100 a minute, 6,000 an hour, 144,000 a day.
Measured on 12 Nov 2026, testnet made a block every 587 ms and mainnet every
570 ms, over 1,000 consecutive blocks each, so real durations run about 2%
to 5% shorter than the round figures below.

The live preset was chosen on 12 Nov 2026 from the simulator's files in
`sim/out/`. One value moved from the spec's first placeholder:
`ARREST_BLOCKS`, from 144,000 to 72,000.

## Why ARREST_BLOCKS is 72,000

The simulator ran six scenarios 1,000 times each over 60 days for three
candidate windows. Only the 12-hour window met all four design goals.

| Goal | 24 hours | 12 hours | 6 hours |
| --- | --- | --- | --- |
| A healthy coin has zero arrests in 60 days | 100% of runs | 100% of runs | 86% of runs |
| A fading coin's median death falls between day 20 and day 50 | 19.5% dead by day 60 | day 45 | day 28 |
| A realistic pair needs 4 to 12 minutes for a save | 4.3 min, 10.7 with 5 scars | same | same |
| A serial rescuer takes under 25% of the cart | 17.9% | 17.9% | 17.9% |

Files: `sim/out/goals.csv` for 12 hours, `sim/out/presets/live-24h/goals.csv`
and `sim/out/presets/live-6h/goals.csv` for the other two.

The result depends on how busy the community is. The scenarios assume 120
holders making 60 transfers a day at launch (`sim/out/scenarios.csv`).
`sim/out/sweep.csv` reruns the healthy and the fading coin for six windows and
eight launch rates. The rule of thumb it gives: the window should be the time
in which the launch community makes 20 to 40 beats. A quieter coin, 10 to 20
transfers a day, would have been better served by 24 hours.

## Values that differ by preset

| Parameter | Demo | Live | Meaning | Reason |
| --- | --- | --- | --- | --- |
| `ARREST_BLOCKS` | 100 | 72,000 | Silence that stops the heart. Live is 12 hours. | Live: see above. Demo: one minute, so the arrest fits in a video. |
| `MIN_STROKE` | 1 ROSC | 100 ROSC | Smallest transfer that is a beat. | Keeps dust from counting. Pacing still costs only 4 ROSC a day (`sim/out/summary.csv`, `paced`), which the design accepts and the monitor labels. |
| `REFRACTORY_BLOCKS` | 5 | 100 | Gap between two beats from one sender. | One wallet cannot fill the trace alone. Live is one minute. |
| `BUCKET_BLOCKS` | 10 | 6,000 | Width of one monitor bucket. Live is 1 hour. | 64 buckets cover the last 64 hours on the live monitor. |
| `NO_FLOW_MAX_BLOCKS` | 1,000 | 12,000 | Arrest with no compressions before death may be called. Live is 2 hours. | Long enough for a pager to reach someone: with rescuers who respond after a median of 10 minutes, 93.5% of codes in the fading scenario got a first compression in time (`sim/out/fading.codes.csv`). |
| `CYCLES_PER_ROUND` | 2 | 5 | Cycles between rhythm checks. | Live: five cycles of 30 and 2 take about 2 minutes, the rhythm-check interval in the AHA algorithm. Demo: two, so a round takes 40 seconds. |
| `LAZARUS_BLOCKS` | 50 | 6,000 | Wait after a death before a new code. Live is 1 hour. | A pause between codes, so three deaths cannot be collected in one sitting. |
| `REHAB_BLOCKS` | 1,000 | 1,008,000 | Unbroken life that heals one scar. Live is 7 days. | Slow enough that a coin saved daily keeps accumulating damage. |

## Values shared by both presets

| Parameter | Value | Meaning | Reason |
| --- | --- | --- | --- |
| `COMPRESSIONS_PER_CYCLE` | 30 | Counted compressions that close a cycle. | The 30:2 ratio with no advanced airway. |
| `BREATHS_PER_CYCLE` | 2 | Breaths in a full cycle. | The same ratio. |
| `VENT_WINDOW_BLOCKS` | 16 | Blocks for the two breaths, and for a shock or a dose after a round. | About 10 seconds, the longest pause in compressions the guidelines allow. |
| `MAX_PAUSE_BLOCKS` | 100 | Pause in compressions after which death may be called. | One minute. Must exceed two windows (34 blocks), which `Config::validate` enforces. |
| `MAX_ROUNDS` | 10 | Rounds in a code. | About 20 minutes of CPR on the live preset. |
| `SUDDEN_BEATS` | 8 | Beats in the last bucket-width before the silence that make the arrest VF. | A coin that was busy and stopped at once. |
| `PEA_MIN` | 3 | Non-beat transfers during the silence that make the arrest PEA. | Activity with no pulse. |
| `VF_DEGRADE_ROUND` | 4 | Round at which an unshocked VF becomes asystole. | VF does not last. Three chances to shock. |
| `BASE_TARGET` | 200 | Perfusion points for ROSC on a first arrest. | Two perfect rounds. |
| `SCAR_TARGET` | 50 | Extra points per scar. | Half a perfect round per earlier arrest. Caps the arrests a coin can survive at 21. |
| `NOFLOW_TARGET_CAP` | 200 | Most the delay can add to the target. | Reached after 2,000 blocks with no compressions. |
| `NOFLOW_TARGET_BLOCKS` | 10 | Blocks of delay per extra point. | The spec's `no_flow_blocks / 10`. |
| `HANDS_ONLY_BPS` | 7,000 | Score of a cycle with no breaths. | Hands-only CPR works, at 70%. |
| `BREATH_BPS` | 1,500 | Extra score per counted breath. | Two breaths make a full cycle. |
| `FATIGUE_BPS` | 8,000 | Round multiplier when the same account leads two rounds running. | Swap the compressor every 2 minutes. |
| `EPI_BONUS` | 50 | Points per dose. | Half a perfect round. |
| `EPI_FIRST_ROUND`, `EPI_EVERY_ROUNDS` | 2, 2 | A dose after round 2 and every second round. | About every 4 minutes on the live preset; the guideline is every 3 to 5. |
| `SHOCK_BONUS` | 150 | Points for a shock that works. | A good round and one shock revive a first VF arrest. |
| `SHOCK_MIN_QUALITY_BPS` | 6,000 | Round quality a shock needs. | A shock does not replace compressions. |
| `O2_BPS` | 50 | Oxygen burn on each live transfer. | 0.5%, paid by the sender on top. |
| `PREMIUM_BPS` | 50 | Premium into the crash cart on each live transfer. | 0.5%, paid by the sender on top. |
| `NOFLOW_BURN_BPS`, `NOFLOW_BURN_BLOCKS` | 1, 100 | Share of the cart burned per 100 blocks with no compressions. | Delay costs the community something. At most 1.2% of the cart on the live preset. |
| `EPI_DOSE` | 1,000,000 ROSC | Burned from the cart per dose. | At most five a code. |
| `DEATH_BURN_BPS` | 1,000 | Share of the cart burned at each time of death. | 10%. |
| `MAX_DEATHS` | 3 | Deaths after which the coin is deceased for good. | Three strikes. |
| `ROSC_SCARS`, `DEATH_SCARS` | 1, 2 | Scars added by a ROSC and by a death. | Every arrest leaves damage. |
| `BOUNTY_BPS` | 200 | Share of the cart paid at ROSC, before the ejection fraction. | 2%. A serial rescuer takes 17.9% of the cart over the coin's life (`sim/out/summary.csv`, `farmed`). |
| `EF_START`, `EF_STEP`, `EF_FLOOR` | 60, 5, 20 | Ejection fraction: 60%, less 5 per scar, never under 20. | The bounty shrinks with damage and heals with rehab. |
| `UNITS_COMPRESSION`, `UNITS_BREATH`, `UNITS_SHOCK`, `UNITS_CALL_CODE` | 1, 5, 15, 10 | Work units that split the bounty. | The spec's table. |
| `TOTAL_SUPPLY` | 2,500,000,000 ROSC | Minted once, at launch. | Close to the beats in 70 years at 70 a minute. |
| `CART_BPS` | 4,000 | Share of the supply minted into the crash cart. | 40% cart, 60% faucet. No team allocation. |
| `FAUCET_AMOUNT` | 1,000,000 ROSC | Paid once per account by `faucet_claim()`. | 1,500 claims. |
| `MIN_RESCUER_BALANCE` | 1,000 ROSC | Balance an account needs to act in a code. | A faucet claim covers it a thousand times over. |
| `POST_MORTEM_TRANSFERS` | false | Reopens plain transfers after final death. | Off: death is final. A deployer who needs tokens to stay movable sets it at launch. |

The scoring numbers borrow the shape of the adult cardiac arrest algorithm and
nothing more. They are game design, not medicine.
