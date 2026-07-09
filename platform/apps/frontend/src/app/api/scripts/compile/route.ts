import { NextResponse } from 'next/server';

export const dynamic = 'force-dynamic';

// Lazy import so Next doesn't bundle solc unless needed
async function compileTS(source: string) {
  const ts = await import('typescript');
  const out = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2020,
      module: ts.ModuleKind.ESNext,
      strict: true,
      jsx: ts.JsxEmit.React,
    },
    reportDiagnostics: true,
  });
  const diags = out.diagnostics?.map(d =>
    ts.flattenDiagnosticMessageText(d.messageText, '\n')
  ) ?? [];
  if (diags.length) throw new Error(diags.join('\n'));
  return out.outputText;
}

async function compileSol(source: string) {
  const solc = await import('solc'); // ensure "solc" is installed
  const input = {
    language: 'Solidity',
    sources: { 'Main.sol': { content: source } },
    settings: { outputSelection: { '*': { '*': ['*'] } } },
  };
  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  if (output.errors?.length) {
    const errs = output.errors.filter((e: any) => e.severity !== 'warning');
    if (errs.length) throw new Error(errs.map((e: any) => e.formattedMessage).join('\n'));
  }
  return output;
}

export async function POST(req: Request) {
  try {
    const { searchParams } = new URL(req.url);
    const lang = (searchParams.get('lang') || 'ts').toLowerCase();
    const code = await req.text();

    const result = lang === 'solidity' || lang === 'sol'
      ? await compileSol(code)
      : await compileTS(code);

    return NextResponse.json({ ok: true, output: result });
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: String(e) }, { status: 400 });
  }
}
