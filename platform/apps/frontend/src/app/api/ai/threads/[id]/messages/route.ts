import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { db } from "@/db/client";
import { aiThreads } from "@/db/schema";
import { eq, and } from "drizzle-orm";
import { GoogleGenerativeAI } from "@google/generative-ai";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type ClientChatRole = "user" | "assistant";
type ClientChatMessage = { role: ClientChatRole; text: string };

function safeJson(v: unknown) {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

/**
 * We trim conversation by approximate character budget to avoid token blowups.
 * This is deliberately simple & robust.
 */
function trimHistory(history: ClientChatMessage[], charBudget = 14000) {
  const out: ClientChatMessage[] = [];
  let used = 0;

  // Keep newest messages first
  for (let i = history.length - 1; i >= 0; i--) {
    const m = history[i];
    const chunk = `${m.role}: ${m.text}\n`;
    if (used + chunk.length > charBudget) break;
    out.unshift(m);
    used += chunk.length;
  }
  return out;
}

export async function POST(
  req: Request,
  { params }: { params: { id: string } }
) {
  const session = await getSession();
  if (!session?.user?.id) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const threadId = params.id;

  // Ensure thread belongs to the user (keeps your existing DB model intact)
  const [thread] = await db
    .select()
    .from(aiThreads)
    .where(and(eq(aiThreads.id, threadId), eq(aiThreads.userId, session.user.id)));

  if (!thread) {
    return NextResponse.json({ error: "Thread not found" }, { status: 404 });
  }

  const body = await req.json().catch(() => ({}));

  const userMessage: string = body?.message ?? "";
  const history: ClientChatMessage[] = Array.isArray(body?.history) ? body.history : [];
  const studioSnapshot = body?.studioSnapshot ?? {};
  const projectMemory = body?.projectMemory ?? {};

  if (!userMessage.trim()) {
    return NextResponse.json({ error: "Message is required" }, { status: 400 });
  }

  const apiKey = process.env.GEMINI_API_KEY;
  if (!apiKey) {
    return NextResponse.json(
      { error: "Server misconfigured: GEMINI_API_KEY missing" },
      { status: 500 }
    );
  }

  // Gemini Flash (stable)
  // Google’s models page shows stable naming like `gemini-2.5-flash`. :contentReference[oaicite:2]{index=2}
  const modelName = process.env.GEMINI_MODEL || "gemini-2.5-flash";

  // Build the prompt: CeruleAI is state-aware, and MUST NOT mention “demo”.
  const trimmed = trimHistory(history, 14000);

  const systemInstruction = `
You are CeruleAI, the built-in assistant inside Cerulea Studio.

Rules:
- Do NOT mention the words "demo", "testing", "fake", or anything similar.
- Speak like a helpful product assistant for non-technical users, but you should be precise.
- Use the provided Studio context to answer. If context is missing, ask ONE clarifying question.
- Keep answers structured and actionable (bullets/steps are fine).
- Do not invent features the UI doesn't have.
`.trim();

  const contextBlock = `
[PROJECT MEMORY]
${safeJson(projectMemory)}

[CURRENT STUDIO SNAPSHOT]
${safeJson(studioSnapshot)}
`.trim();

  const conversationBlock = trimmed
    .map((m) => (m.role === "user" ? `User: ${m.text}` : `CeruleAI: ${m.text}`))
    .join("\n");

  const finalPrompt = `
${systemInstruction}

${contextBlock}

[RECENT CONVERSATION]
${conversationBlock}

[USER MESSAGE]
${userMessage}

Now respond as CeruleAI:
`.trim();

  try {
    const genAI = new GoogleGenerativeAI(apiKey);
    const model = genAI.getGenerativeModel({ model: modelName });

    const result = await model.generateContent(finalPrompt);
    const text = result.response.text() || "";

    return NextResponse.json({ reply: text });
  } catch (err: any) {
    const msg =
      typeof err?.message === "string"
        ? err.message
        : "Gemini request failed";
    return NextResponse.json({ error: msg }, { status: 500 });
  }
}
