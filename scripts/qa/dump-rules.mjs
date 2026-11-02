import { ApiPromise, WsProvider } from '../../apps/web/node_modules/@polkadot/api/index.js';
const api = await ApiPromise.create({ provider: new WsProvider('ws://127.0.0.1:9944'), noInitWarn: true });
const def = await api.query.pramaanRuleRegistry.defaultRule();
console.log('DEFAULT RULE:', JSON.stringify(def.toJSON()));
const entries = await api.query.pramaanRuleRegistry.rules.entries();
for (const [k, v] of entries) {
  const id = Buffer.from(k.args[0].toU8a(true)).toString('utf8');
  const j = v.toJSON();
  console.log(id.padEnd(14), JSON.stringify(j.hsnThresholds), 'margin=', j.preferenceMarginBps, 'method=', j.calculationMethod, 'div=', j.divisibility, 'para3a=', j.para3aApplicable, 'pli=', j.pliLinked, 'certThr=', j.certificationThreshold);
}
const hdr = await api.rpc.chain.getHeader();
const fin = await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead());
console.log('best=', hdr.number.toNumber(), 'finalized=', fin.number.toNumber());
process.exit(0);
