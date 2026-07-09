// apps/frontend/src/app/api/compile/solidity/route.ts
import { NextRequest, NextResponse } from 'next/server';

export async function POST(req: NextRequest) {
  try {
    const { source } = await req.json();
    if (!source || typeof source !== 'string') {
      return NextResponse.json({ ok: false, error: 'source required' }, { status: 400 });
    }
    // Lazy import so client never bundles it
    const solc: any = await import('solc');
    const input = {
      language: 'Solidity',
      sources: { 'input.sol': { content: source } },
      settings: { outputSelection: { '*': { '*': ['abi', 'evm.bytecode'] } } },
    };
    const output = JSON.parse(solc.compile(JSON.stringify(input)));

    if (output.errors?.some((e: any) => e.severity === 'error')) {
      const messages = output.errors
        .filter((e: any) => e.severity === 'error')
        .map((e: any) => e.formattedMessage);
      return NextResponse.json({ ok: false, error: messages.join('\n') }, { status: 400 });
    }

    return NextResponse.json({ ok: true, output });
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || 'compile failed' }, { status: 500 });
  }
}
