import { ApiPromise, WsProvider } from '../../apps/web/node_modules/@polkadot/api/index.js';
const api = await ApiPromise.create({ provider: new WsProvider('ws://127.0.0.1:9944'), noInitWarn: true });
const now = (await api.rpc.chain.getHeader()).number.toNumber();
const entries = await api.query.pramaanDebarment.debarments.entries();
console.log('current block', now);
for (const [k, v] of entries) {
  const acct = k.args[0].toString();
  for (const r of v.toJSON()) {
    const active = r.effectiveFrom <= now && (r.effectiveTo === null || r.effectiveTo > now);
    if (active) console.log(' ACTIVE', acct, 'ministry=', Buffer.from(r.ministry.slice(2),'hex').toString(), 'reason=', Buffer.from(r.reason.slice(2),'hex').toString().slice(0,50));
  }
}
process.exit(0);
