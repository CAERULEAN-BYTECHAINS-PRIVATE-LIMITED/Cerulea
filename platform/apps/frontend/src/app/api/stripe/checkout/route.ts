import { NextResponse } from 'next/server';
import Stripe from 'stripe';
import { getSession } from '@/lib/auth';
import { db } from '@/db/client';
import { subscriptions } from '@/db/schema';
import { eq } from 'drizzle-orm';
import { randomUUID } from 'crypto';

export const dynamic = 'force-dynamic';

export async function POST(req: Request) {
  try {
    const session = await getSession();
    if (!session?.user?.id) {
      return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });
    }

    const body = await req.json();
    const { plan, returnUrl } = body;

    // Dev-mode fallback: Stripe not configured → directly activate subscription
    if (!process.env.STRIPE_SECRET_KEY) {
      if (!plan || !['developer', 'pro'].includes(plan)) {
        return NextResponse.json({ ok: false, error: 'Invalid plan' }, { status: 400 });
      }
      const [existing] = await db.select({ id: subscriptions.id }).from(subscriptions).where(eq(subscriptions.userId, session.user.id)).limit(1);
      const periodEnd = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString();
      if (existing) {
        await db.update(subscriptions).set({ plan, status: 'active', currentPeriodEnd: periodEnd, updatedAt: new Date().toISOString() }).where(eq(subscriptions.userId, session.user.id));
      } else {
        await db.insert(subscriptions).values({ id: randomUUID(), userId: session.user.id, plan, status: 'active', currentPeriodEnd: periodEnd });
      }
      const origin = req.headers.get('origin') || 'http://localhost:3000';
      // Redirect to /pricing/success so the client can force-refresh the JWT token
      const successUrl = returnUrl
        ? `${origin}/pricing/success?return=${encodeURIComponent(returnUrl)}`
        : `${origin}/pricing/success`;
      return NextResponse.json({ ok: true, url: successUrl, devMode: true });
    }

    const stripe = new Stripe(process.env.STRIPE_SECRET_KEY, {
      apiVersion: '2026-03-25.dahlia',
    });

    const PRICE_IDS: Record<string, string> = {
      developer: process.env.STRIPE_DEVELOPER_PRICE_ID || '',
      pro: process.env.STRIPE_PRO_PRICE_ID || '',
    };

    if (!plan || !PRICE_IDS[plan]) {
      return NextResponse.json({ ok: false, error: 'Invalid plan' }, { status: 400 });
    }

    const origin = req.headers.get('origin') || process.env.NEXTAUTH_URL || 'http://localhost:3000';
    const successBase = `${origin}/pricing/success?session_id={CHECKOUT_SESSION_ID}`;
    const successUrl = returnUrl ? `${successBase}&return=${encodeURIComponent(returnUrl)}` : successBase;

    const checkoutSession = await stripe.checkout.sessions.create({
      mode: 'subscription',
      payment_method_types: ['card'],
      line_items: [
        {
          price: PRICE_IDS[plan],
          quantity: 1,
        },
      ],
      metadata: {
        userId: session.user.id,
        plan,
        returnUrl: returnUrl || '',
      },
      customer_email: session.user.email,
      success_url: successUrl,
      cancel_url: `${origin}/pricing`,
    });

    return NextResponse.json({ ok: true, url: checkoutSession.url });
  } catch (err: any) {
    console.error('Stripe checkout error:', err);
    return NextResponse.json({ ok: false, error: err.message || 'Checkout failed' }, { status: 500 });
  }
}
