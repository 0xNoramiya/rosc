const ONE = 10n ** 18n;

/** A base-unit amount as ROSC: 1.00B, 20.0M, 1,010, 0.5. */
export function rosc(amount: string | bigint | number | undefined): string {
  if (amount === undefined) return '0';
  const units = typeof amount === 'bigint' ? amount : BigInt(amount);
  const whole = Number(units / ONE) + Number(units % ONE) / 1e18;
  if (whole >= 1e9) return `${(whole / 1e9).toFixed(2)}B`;
  if (whole >= 1e6) return `${(whole / 1e6).toFixed(1)}M`;
  if (whole >= 100) return Math.round(whole).toLocaleString('en-US');
  return whole.toFixed(whole >= 1 || whole === 0 ? 0 : 2);
}

export function toUnits(whole: string): string {
  const [int, frac = ''] = whole.trim().split('.');
  return (BigInt(int || '0') * ONE + BigInt((frac + '0'.repeat(18)).slice(0, 18) || '0')).toString();
}

/** A stretch of blocks as time: 42 s, 1:40, 2 h 5 min, 3 d 4 h. */
export function span(blocks: number, blockMs: number): string {
  const seconds = Math.max(0, Math.round((blocks * blockMs) / 1000));
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} h ${Math.floor((seconds % 3600) / 60)} min`;
  return `${Math.floor(seconds / 86400)} d ${Math.floor((seconds % 86400) / 3600)} h`;
}

/** Perfusion and targets arrive in hundredths of a point. */
export const points = (hundredths: number) => (hundredths / 100).toFixed(hundredths % 100 ? 1 : 0);

export const short = (account: string | null) =>
  !account ? '' : account.length > 24 ? `${account.slice(0, 10)}…${account.slice(-9)}` : account;

/** The four bytes the contract uses for an account on the trace. */
export async function tagOf(account: string): Promise<number> {
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(account)));
  return (hash[0] | (hash[1] << 8) | (hash[2] << 16) | (hash[3] << 24)) >>> 0;
}
