// The page's acceptance list, against testnet.
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '../..');
const deployment = (name: string) => JSON.parse(readFileSync(join(repo, 'deployments', `${name}.json`), 'utf8'));

async function chain(page: Page) {
  return page.evaluate(() => (window as any).codeblue as { monitor: any; reading: any });
}

async function ready(page: Page) {
  await page.waitForFunction(() => (window as any).codeblue?.monitor, null, { timeout: 60_000 });
}

// Against testnet-live, the one coin on file that stays alive: a scheduled
// pacemaker beats it. A demo-preset coin is dead half an hour after its pacer
// stops, and a dead coin has a headstone where the trace was.
test('the trace matches the beat ring', async ({ page }) => {
  await page.goto('/?coin=testnet-live#monitor');
  await ready(page);
  const { monitor, reading } = await chain(page);
  const drawn = reading.marks.filter((m: any) => m.kind === 'beat').map((m: any) => [m.block, m.tag]);
  expect(drawn).toEqual(monitor.beats.map((b: any) => [b.block, b.tag]));
  expect(monitor.beats.length).toBeGreaterThan(0);
  await expect(page.locator('canvas.ecg')).toHaveAttribute('aria-label', new RegExp(`${monitor.beats.length} beats in the ring`));
});

test('the headstone renders from a deceased deployment', async ({ page }) => {
  await page.goto('/?coin=testnet-headstone#monitor');
  await ready(page);
  const { monitor } = await chain(page);
  const block = monitor.status.Deceased.block;
  await expect(page.getByRole('article', { name: 'Headstone' })).toBeVisible();
  await expect(page.getByText(`Time of death: block ${block.toLocaleString('en-US')}`)).toBeVisible();
  await expect(page.getByRole('status').filter({ hasText: 'Deceased' })).toBeVisible();
  await expect(page.getByText('Arrests survived')).toBeVisible();
});

test('the parameters and the simulation screens read their sources', async ({ page }) => {
  await page.goto('/?coin=testnet-demo#parameters');
  await ready(page);
  await expect(page.getByRole('row', { name: /ARREST_BLOCKS/ })).toContainText('100 blocks, about');
  await page.goto('/?coin=testnet-demo#simulation');
  await expect(page.getByRole('heading', { name: 'Survival' })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'A healthy coin has zero arrests in 60 days' })).toBeVisible();
  await expect(page.locator('.sim-grid svg').first()).toBeVisible();
});

// A whole code from the keyboard, alone, on a coin deployed for this test.
// Needs CODE_COIN: the name of a deployment whose first bot the test may
// drive (`node bots/deploy.mjs <name>`). Skipped when it is not set.
test('keyboard-only use completes a full code with no wallet prompt', async ({ page, context }) => {
  const name = process.env.CODE_COIN;
  test.skip(!name, 'set CODE_COIN to a fresh deployment');
  const coin = deployment(name!);
  const bots: string[] = coin.bots.map((b: string | { id: string }) => (typeof b === 'string' ? b : b.id));
  const rescuer: string = process.env.CODE_RESCUER ?? bots.find(b => b.startsWith('bot-compressor'))!;
  const keyFile = [join(repo, 'bots/.keys', `${rescuer}.json`), join(process.env.HOME!, '.near-credentials/testnet', `${rescuer}.json`)].find(existsSync);
  expect(keyFile, `a key for ${rescuer}`).toBeTruthy();
  const key = JSON.parse(readFileSync(keyFile!, 'utf8'));
  const secret: string = key.private_key ?? key.secretKey ?? key.secret_key;

  let prompts = 0;
  context.on('page', () => prompts++);
  page.on('dialog', () => prompts++);

  await page.goto(`/?coin=${name}#code`);
  await ready(page);
  // Scrub in from the keyboard with a key kept on this device.
  await page.getByText('Use a key kept on this device instead').focus();
  await page.keyboard.press('Enter');
  await page.getByLabel('Account').focus();
  await page.keyboard.type(rescuer);
  await page.keyboard.press('Tab');
  await page.keyboard.type(secret);
  await page.keyboard.press('Enter');
  await expect(page.getByText('The page holds a key for the rescue calls.')).toBeVisible({ timeout: 30_000 });

  // Wait for the heart to stop.
  await page.waitForFunction(() => (window as any).codeblue.monitor.status === 'Arrest', null, { timeout: 5 * 60_000, polling: 500 });
  await page.keyboard.press('c');

  const blockMs = 600;
  const sentAt: number[] = [];
  let firstThirty: { counted: number; missed: number } | null = null;
  const started = Date.now();
  // Press on the beat until the coin is alive, taking the dose when offered.
  for (;;) {
    const { monitor } = await chain(page);
    if (monitor.status !== 'Arrest') break;
    expect(Date.now() - started, 'the code should end inside ten minutes').toBeLessThan(10 * 60_000);
    const phase = monitor.code.phase;
    if (phase === 'Compressions') {
      await page.keyboard.press('Space');
      sentAt.push(Date.now());
    } else if (typeof phase === 'object' && 'AfterRound' in phase && phase.AfterRound.epi_open) {
      await page.keyboard.press('e');
    }
    if (sentAt.length === 30 && !firstThirty) {
      // Thirty presses are out. Give the last ones two blocks past execution to be marked.
      await page.waitForTimeout(4 * blockMs);
      firstThirty = {
        counted: await page.locator('.press.m-compress.is-counted').count(),
        missed: await page.locator('.press.m-compress.is-missed').count(),
      };
    }
    await page.waitForTimeout(blockMs);
  }

  expect(sentAt.length).toBeGreaterThanOrEqual(30);
  expect(firstThirty, 'thirty presses went out').not.toBeNull();
  // Every one of the first presses is marked one way or the other, and most counted.
  expect(firstThirty!.counted + firstThirty!.missed).toBeGreaterThanOrEqual(26);
  expect(firstThirty!.counted).toBeGreaterThanOrEqual(20);
  // Counted and missed presses are drawn differently: a solid square and a struck outline.
  const counted = page.locator('.press.is-counted').first();
  await expect(counted).toBeVisible();
  expect(await counted.evaluate(el => getComputedStyle(el).backgroundColor)).not.toBe('rgba(0, 0, 0, 0)');

  // The code ended in ROSC.
  const { monitor } = await chain(page);
  expect(typeof monitor.status === 'object' && 'Alive' in monitor.status).toBe(true);
  // A code that ends on a deadline is written by the next transaction, so the
  // sheet may not be on chain yet. The rescuer's next move writes it either
  // way: the claim, from the keyboard, with the same key.
  const claim = page.getByRole('button', { name: /Claim .* ROSC bounty/ });
  await expect(claim).toBeVisible({ timeout: 30_000 });
  await claim.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Bounty claimed.')).toBeVisible({ timeout: 30_000 });
  expect(prompts, 'no wallet window and no dialog opened').toBe(0);
  // The sheet says who did the work.
  await expect(page.getByRole('region', { name: /Code sheet/ })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText('return of spontaneous circulation')).toBeVisible();
  // How long a press waits for its mark. The chain runs a press about one
  // second after it is sent; the mark follows within two blocks of that.
  const waits: number[] = await page.evaluate(() =>
    ((window as any).codebluePresses as any[]).filter(p => p.settledAt).map(p => p.settledAt - p.sentAt).sort((a, b) => a - b),
  );
  const median = waits[Math.floor(waits.length / 2)];
  expect(median, 'press to mark, median').toBeLessThan(4 * blockMs + 1000);
  console.log(
    `full code from the keyboard: ${sentAt.length} presses in ${Math.round((Date.now() - started) / 1000)} s; first thirty ${JSON.stringify(firstThirty)}; ` +
      `press to mark median ${Math.round(median)} ms, slowest ${Math.round(waits[waits.length - 1])} ms`,
  );
});

// The practice coin's guest path: no wallet, a key lent by the guest desk.
// The desk is a server function on the page's host (api/guest.mjs); here it
// runs inside the test, so this needs the guest account's key in bots/.keys
// and is skipped without it. Against the hosted page nothing is stubbed:
// `GUEST_PAGE=https://rosc-eight.vercel.app npx playwright test -g guest`.
test('a guest with no wallet does CPR on the practice coin', async ({ page }) => {
  const coin = deployment('testnet-practice');
  const guest: string = coin.bots.find((b: { role?: string }) => b.role === 'guest').id;
  const hosted = process.env.GUEST_PAGE;
  const keyFile = join(repo, 'bots/.keys', `${guest}.json`);
  test.skip(!hosted && !existsSync(keyFile), `needs ${guest}'s key, or GUEST_PAGE`);
  if (!hosted) {
    process.env.GUEST_KEY = readFileSync(keyFile, 'utf8');
    const { POST } = await import(join(repo, 'api/guest.mjs'));
    await page.route('**/api/guest', async route => {
      const answer: Response = await POST(new Request('http://localhost/api/guest', { method: 'POST', body: route.request().postData(), headers: { 'content-type': 'application/json' } }));
      await route.fulfill({ status: answer.status, contentType: 'application/json', body: await answer.text() });
    });
  }

  await page.goto(`${hosted ?? ''}/?coin=testnet-live#monitor`);
  await page.getByRole('link', { name: 'Do CPR on the practice coin' }).click({ timeout: 60_000 });
  await ready(page);
  const started = Date.now();
  await page.getByRole('button', { name: 'Scrub in as a guest' }).first().click();
  await expect(page.getByText('The page holds a key for the rescue calls.')).toBeVisible({ timeout: 60_000 });
  const scrubbed = Date.now() - started;
  await expect(page.getByText('A guest can do CPR and nothing else.')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Send a beat' })).toHaveCount(0);

  // The coin has a pulse for a minute after each save. Wait for it to stop.
  await page.waitForFunction(() => (window as any).codeblue.monitor.status === 'Arrest', null, { timeout: 3 * 60_000, polling: 500 });
  await page.locator('body').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('c');
  let presses = 0;
  const called = Date.now();
  for (;;) {
    const { monitor } = await chain(page);
    if (monitor.status !== 'Arrest') break;
    expect(Date.now() - called, 'a practice code should end inside three minutes').toBeLessThan(3 * 60_000);
    if (monitor.code.phase === 'Compressions') {
      await page.keyboard.press('Space');
      presses++;
    }
    await page.waitForTimeout(600);
  }
  console.log(`guest: scrubbed in after ${(scrubbed / 1000).toFixed(1)} s; ${presses} presses, pulse back ${((Date.now() - called) / 1000).toFixed(0)} s after the code was called`);
});
