import { NextRequest, NextResponse } from "next/server";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(req: NextRequest) {
  try {
    const { code } = await req.json();
    if (!code || typeof code !== "string") {
      return NextResponse.json({ ok:false, error:"Missing Solidity code" }, { status:400 });
    }
    // server-side import avoids bundling 'fs' in the client
    const solc: any = await import("solc");
    const input = {
      language: "Solidity",
      sources: { "Contract.sol": { content: code } },
      settings: { outputSelection: { "*": { "*": ["abi","evm.bytecode"] } } }
    };
    const output = JSON.parse(solc.compile(JSON.stringify(input)));
    const errs = (output.errors || []).filter((x:any)=>x.severity==="error");
    if (errs.length) {
      const first = errs[0]?.formattedMessage || errs[0]?.message || "Compile error";
      return NextResponse.json({ ok:false, error:first }, { status:400 });
    }
    return NextResponse.json({ ok:true, data: output.contracts?.["Contract.sol"] || {} });
  } catch (e:any) {
    return NextResponse.json({ ok:false, error: e.message ?? String(e) }, { status:500 });
  }
}
