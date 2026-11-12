// near-api-js pulls in `generate-function`, which asks Node for `util.format`.
// Browsers have no such module, so the bundle points "util" here. Only the
// ABI-typed `Contract` class reaches it, and this page does not use that.
export function format(fmt: unknown, ...args: unknown[]): string {
  let i = 0;
  const out = String(fmt).replace(/%[sdj%]/g, m => {
    if (m === '%%') return '%';
    if (i >= args.length) return m;
    const a = args[i++];
    return m === '%j' ? JSON.stringify(a) : m === '%d' ? String(Number(a)) : String(a);
  });
  return [out, ...args.slice(i).map(String)].join(' ');
}
export default { format };
